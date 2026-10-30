//! Credential identity proof, not proof of message receipt or processing.
use super::*;
use crate::domain::{check_id, check_request_id};
use crate::json::{fields, required};
use crate::storage::driver::optional_text;

const CHALLENGE_MS: i64 = 10 * 60 * 1000;
const PROOF_MS: i64 = 24 * 60 * 60 * 1000;

impl<D: Database, O: ObjectStore> App<D, O> {
    fn receiver_challenge_mac(&self, row: &Row) -> Result<Hmac<Sha256>> {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.csrf_key)
            .map_err(|_| Error::internal("receiver proof key"))?;
        mac.update(b"waypost-receiver-proof-v1\0");
        for field in [
            "binding_id",
            "space_id",
            "actor_id",
            "consumer_id",
            "target",
        ] {
            mac.update(text(row, field)?.as_bytes());
            mac.update(b"\0");
        }
        Ok(mac)
    }

    pub(super) async fn receiver_binding_create(
        &self,
        p: &Principal,
        value: Value,
    ) -> Result<Value> {
        fields(&value, &["request_id", "target"])?;
        let request_id = required(&value, "request_id")?;
        check_request_id(&request_id)?;
        let target = required(&value, "target")?;
        if target.is_empty() || target.len() > 512 || target.chars().any(char::is_control) {
            return Err(Error::invalid(
                "Receiver target must be an opaque, nonempty binding of at most 512 bytes.",
            ));
        }
        if p.view != "inbox" {
            return Err(Error::forbidden());
        }
        let now = self.clock.now_ms();
        let (guard, auth) = self.authority_guard(p);
        let mut args = vec![
            random_id("receiver")?.into(),
            p.space_id.clone().into(),
            p.actor_id.clone().into(),
            p.consumer_id.clone().into(),
            request_id.clone().into(),
            target.clone().into(),
            now.into(),
            (now + CHALLENGE_MS).into(),
        ];
        args.extend(auth);
        self.db.batch(vec![Statement::new(format!("INSERT INTO receiver_bindings(binding_id,space_id,actor_id,consumer_id,request_id,target,created_at,expires_at) SELECT ?,?,?,?,?,?,?,? WHERE {guard} AND (SELECT COUNT(*) FROM receiver_bindings WHERE expires_at>?)<10000 ON CONFLICT(space_id,actor_id,consumer_id,request_id) DO NOTHING"), {args.push(now.into()); args})]).await?;
        self.current(p).await?;
        let rows = self.db.query(Statement::new("SELECT * FROM receiver_bindings WHERE space_id=? AND actor_id=? AND consumer_id=? AND request_id=?",vec![p.space_id.clone().into(),p.actor_id.clone().into(),p.consumer_id.clone().into(),request_id.into()])).await?;
        let row = rows
            .first()
            .ok_or_else(|| Error::new("RESOURCE_LIMIT", "Too many active receiver proofs."))?;
        if text(row, "target")? != target {
            return Err(Error::new(
                "IDEMPOTENCY_CONFLICT",
                "The receiver request ID belongs to another target.",
            ));
        }
        let mut result = self.receiver_binding_view(row).await?;
        // Domain-separated HMAC gives idempotent retries without storing a secret.
        result["challenge"] = URL_SAFE_NO_PAD
            .encode(self.receiver_challenge_mac(row)?.finalize().into_bytes())
            .into();
        Ok(result)
    }

    pub(super) async fn receiver_binding_status(&self, p: &Principal, id: &str) -> Result<Value> {
        check_id(id)?;
        self.current(p).await?;
        let rows = self.db.query(Statement::new("SELECT * FROM receiver_bindings WHERE binding_id=? AND space_id=? AND actor_id=? AND consumer_id=?",vec![id.into(),p.space_id.clone().into(),p.actor_id.clone().into(),p.consumer_id.clone().into()])).await?;
        let row = rows.first().ok_or_else(Error::not_found)?;
        self.receiver_binding_view(row).await
    }

    async fn receiver_binding_view(&self, row: &Row) -> Result<Value> {
        let expires = integer(row, "expires_at")?;
        let mut status = if expires <= self.clock.now_ms() {
            "expired"
        } else if optional_text(row, "receiver_token_id").is_some() {
            "verified"
        } else {
            "pending"
        };
        // A revoked/disabled receiver credential invalidates proof immediately.
        // OAuth refresh creates a new token_id but preserves the authorization family.
        // Keep the original token as a family reference; never read a host token.
        if status == "verified" {
            let (guard, guard_args) = self.oauth_authority_guard(self.clock.now_ms());
            let mut args = vec![
                text(row, "receiver_token_id")?.into(),
                self.clock.now_ms().into(),
            ];
            args.extend(guard_args);
            let query = crate::storage::queries::AUTHENTICATE
                .replace("t.token_digest=?", "t.token_id=?")
                .replace(
                    "(t.expires_at IS NULL OR t.expires_at>?)",
                    "(t.credential_kind='oauth' OR t.expires_at IS NULL OR t.expires_at>?)",
                );
            let live = self
                .db
                .query(Statement::new(format!("{query} AND {guard}"), args))
                .await?;
            if live.is_empty() {
                status = "expired";
            }
        }
        Ok(
            json!({"binding_id":text(row,"binding_id")?,"target":text(row,"target")?,"status":status,"expires_at":expires}),
        )
    }

    pub(super) async fn receiver_confirm(&self, p: &Principal, value: Value) -> Result<Value> {
        fields(&value, &["binding_id", "challenge"])?;
        let id = required(&value, "binding_id")?;
        check_id(&id)?;
        let challenge = required(&value, "challenge")?;
        if challenge.len() > 128 {
            return Err(Error::invalid("Invalid receiver proof."));
        }
        let rows = self
            .db
            .query(Statement::new(
                "SELECT * FROM receiver_bindings WHERE binding_id=?",
                vec![id.clone().into()],
            ))
            .await?;
        let row = rows.first().ok_or_else(Error::not_found)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(challenge)
            .map_err(|_| Error::invalid("Invalid receiver proof."))?;
        self.receiver_challenge_mac(row)?
            .verify_slice(&bytes)
            .map_err(|_| Error::not_found())?;
        if p.space_id != text(row, "space_id")?
            || p.actor_id != text(row, "actor_id")?
            || p.consumer_id != text(row, "consumer_id")?
            || p.view != "inbox"
        {
            return Err(Error::new(
                "RECEIVER_IDENTITY_MISMATCH",
                "This connection is not the receiver identity expected by the bridge. Reconnect the target session to the intended assistant before confirming.",
            ));
        }
        let now = self.clock.now_ms();
        let (guard, auth) = self.authority_guard(p);
        let mut args = vec![
            now.into(),
            (now + PROOF_MS).into(),
            p.token_id.clone().into(),
            id.clone().into(),
            now.into(),
        ];
        args.extend(auth);
        let updated=self.db.batch(vec![Statement::new(format!("UPDATE receiver_bindings SET confirmed_at=?,expires_at=?,receiver_token_id=? WHERE binding_id=? AND confirmed_at IS NULL AND expires_at>? AND {guard} RETURNING binding_id"),args)]).await?;
        self.current(p).await?;
        if updated[0].is_empty()
            && (optional_text(row, "receiver_token_id").is_none()
                || integer(row, "expires_at")? <= now)
        {
            return Err(Error::new(
                "RECEIVER_PROOF_EXPIRED",
                "Receiver proof expired or was confirmed by another connection. Start a new pairing request.",
            ));
        }
        self.receiver_binding_status(p, &id).await
    }
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use crate::native::{FileStore, SqliteDb};
    use std::cell::Cell;

    struct TestClock(Cell<i64>);
    impl Clock for TestClock {
        fn now_ms(&self) -> i64 {
            self.0.get()
        }
    }

    async fn fixture() -> (
        tempfile::TempDir,
        App<SqliteDb, FileStore>,
        Principal,
        Principal,
        Rc<TestClock>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let clock = Rc::new(TestClock(Cell::new(1_000_000)));
        let app = App::new(
            SqliteDb::open(dir.path().join("db")).unwrap(),
            FileStore::open(dir.path().join("objects")).unwrap(),
            Config {
                admin_secret: "receiver-test-admin-secret-more-than32".into(),
                public_origin: "http://localhost".into(),
            },
        )
        .with_clock(clock.clone());
        let body = json!({"request_id":"setup","display_name":"Receiver proof fixture"});
        let space = app
            .admin(
                &Request::json("POST", "/admin/v0/spaces", "unused", body.clone()),
                body,
            )
            .await
            .unwrap();
        let mut principals = vec![];
        for rid in ["bridge", "receiver"] {
            let body = json!({"request_id":rid,"actor_id":space["default_actor_id"],"consumer_id":space["default_consumer_id"]});
            let token = app
                .admin(
                    &Request::json(
                        "POST",
                        &format!(
                            "/admin/v0/spaces/{}/tokens",
                            space["space_id"].as_str().unwrap()
                        ),
                        "unused",
                        body.clone(),
                    ),
                    body,
                )
                .await
                .unwrap();
            let r = Request::json(
                "GET",
                "/v0/describe",
                token["token"].as_str().unwrap(),
                json!({}),
            );
            principals.push(app.authenticate(&r).await.unwrap());
        }
        let receiver = principals.pop().unwrap();
        let bridge = principals.pop().unwrap();
        (dir, app, bridge, receiver, clock)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn receiver_proof_is_idempotent_identity_bound_and_not_a_receipt() {
        let (_dir, app, bridge, receiver, clock) = fixture().await;
        let body = json!({"request_id":"pair1","target":"thread_hash"});
        let first = app
            .receiver_binding_create(&bridge, body.clone())
            .await
            .unwrap();
        let repeat = app.receiver_binding_create(&bridge, body).await.unwrap();
        assert_eq!(first, repeat);
        let id = first["binding_id"].as_str().unwrap();
        let confirm = json!({"binding_id":id,"challenge":first["challenge"]});
        let mut wrong = receiver.clone();
        wrong.actor_id = "another".into();
        assert_eq!(
            app.receiver_confirm(&wrong, confirm.clone())
                .await
                .unwrap_err()
                .code,
            "RECEIVER_IDENTITY_MISMATCH"
        );
        assert_eq!(
            app.receiver_binding_status(&bridge, id).await.unwrap()["status"],
            "pending"
        );
        assert_eq!(
            app.receiver_confirm(&receiver, confirm.clone())
                .await
                .unwrap()["status"],
            "verified"
        );
        assert_eq!(
            app.receiver_confirm(&receiver, confirm).await.unwrap()["status"],
            "verified"
        );
        let counts = app
            .db
            .query(Statement::new("SELECT COUNT(*) AS n FROM receipts", vec![]))
            .await
            .unwrap();
        assert_eq!(integer(&counts[0], "n").unwrap(), 0);
        clock.0.set(clock.now_ms() + PROOF_MS);
        assert_eq!(
            app.receiver_binding_status(&bridge, id).await.unwrap()["status"],
            "expired"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn receiver_proof_rejects_wrong_challenge_changed_target_and_revocation() {
        let (_dir, app, bridge, receiver, _clock) = fixture().await;
        let proof = app
            .receiver_binding_create(&bridge, json!({"request_id":"pair1","target":"thread_one"}))
            .await
            .unwrap();
        assert_eq!(
            app.receiver_binding_create(
                &bridge,
                json!({"request_id":"pair1","target":"thread_two"})
            )
            .await
            .unwrap_err()
            .code,
            "IDEMPOTENCY_CONFLICT"
        );
        let id = proof["binding_id"].as_str().unwrap();
        assert!(
            app.receiver_confirm(&receiver, json!({"binding_id":id,"challenge":"wrong"}))
                .await
                .is_err()
        );
        app.receiver_confirm(
            &receiver,
            json!({"binding_id":id,"challenge":proof["challenge"]}),
        )
        .await
        .unwrap();
        app.db
            .batch(vec![Statement::new(
                "UPDATE tokens SET revoked_at=? WHERE token_id=?",
                vec![1_000_001i64.into(), receiver.token_id.clone().into()],
            )])
            .await
            .unwrap();
        assert_eq!(
            app.receiver_binding_status(&bridge, id).await.unwrap()["status"],
            "expired"
        );
    }
}
