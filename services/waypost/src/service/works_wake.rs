//! Independent bridge-to-session pairing and reference-only wake polling.
use super::works_v2::WorksPrincipal;
use super::*;
use crate::domain::{check_id, check_request_id};
use crate::json::{fields, required};
use crate::storage::driver::optional_text;

const CHALLENGE_MS: i64 = 10 * 60 * 1000;
const PROOF_MS: i64 = 24 * 60 * 60 * 1000;

impl<D: Database, O: ObjectStore> App<D, O> {
    fn wake_mac(&self, row: &Row) -> Result<Hmac<Sha256>> {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.csrf_key)
            .map_err(|_| Error::internal("wake proof key"))?;
        mac.update(b"works-wake-proof-v1\0");
        for key in [
            "binding_id",
            "account_id",
            "assistant_id",
            "bridge_installation_id",
            "bridge_family_id",
            "bridge_attachment_id",
            "execution_attachment_id",
            "session_id",
            "execution_installation_id",
            "target",
        ] {
            mac.update(text(row, key)?.as_bytes());
            mac.update(b"\0");
        }
        mac.update(&integer(row, "registration_revision")?.to_be_bytes());
        Ok(mac)
    }

    // All mutable authority is checked within the same SQL statement that reads
    // assignments or confirms proof; old families and attachments fail closed.
    fn wake_guard(&self, bridge: bool, now: i64) -> (String, Vec<Sql>) {
        let (bg, ba) = self.oauth_client_guard("bf", now);
        let (rg, ra) = self.oauth_client_guard("rf", now);
        let (identity, ia) = self.unified_identity_guard("account");
        let receiver = if bridge {
            format!(
                "AND b.confirmed_at IS NOT NULL AND rf.family_id=b.receiver_family_id AND rf.installation_id=b.execution_installation_id AND rf.account_id=b.account_id AND rf.revoked_at IS NULL AND rf.expires_at>? AND rf.resource=? AND {rg}"
            )
        } else {
            String::new()
        };
        let mut args: Vec<Sql> = vec![now.into(), self.oauth_resource().into()];
        args.extend(ba);
        if bridge {
            args.push(now.into());
            args.push(self.oauth_resource().into());
            args.extend(ra);
        }
        args.extend(ia);
        (
            format!(
                "b.expires_at>? AND bf.family_id=b.bridge_family_id AND bf.account_id=b.account_id AND bf.installation_id=b.bridge_installation_id AND bf.revoked_at IS NULL AND bf.expires_at>? AND bf.resource=? AND {bg} {receiver} AND account.disabled=0 AND {identity} AND a.status='active' AND a.messages_enabled=1 AND bi.status='active' AND ei.status='active' AND bi.installation_id<>ei.installation_id AND bm.status='active' AND em.status='active' AND bm.attachment_id=b.bridge_attachment_id AND em.attachment_id=b.execution_attachment_id AND s.status='registered' AND s.capability='wakeable' AND s.route_revision=b.registration_revision AND s.installation_id=b.execution_installation_id"
            ),
            {
                let mut all = vec![now.into()];
                all.extend(args);
                all
            },
        )
    }
    fn wake_from() -> &'static str {
        " FROM works_wake_bindings b JOIN works_families bf ON bf.family_id=b.bridge_family_id LEFT JOIN works_families rf ON rf.family_id=b.receiver_family_id JOIN accounts account ON account.account_id=b.account_id JOIN works_assistants a ON a.account_id=b.account_id AND a.assistant_id=b.assistant_id JOIN works_installations bi ON bi.account_id=b.account_id AND bi.installation_id=b.bridge_installation_id JOIN works_installations ei ON ei.account_id=b.account_id AND ei.installation_id=b.execution_installation_id JOIN works_attachments bm ON bm.account_id=b.account_id AND bm.assistant_id=b.assistant_id AND bm.installation_id=b.bridge_installation_id JOIN works_attachments em ON em.account_id=b.account_id AND em.assistant_id=b.assistant_id AND em.installation_id=b.execution_installation_id JOIN works_sessions s ON s.account_id=b.account_id AND s.assistant_id=b.assistant_id AND s.session_id=b.session_id "
    }
    async fn wake_live(&self, p: &WorksPrincipal, id: &str, verified: bool) -> Result<Row> {
        let (g, ga) = self.wake_guard(verified, self.clock.now_ms());
        let mut args = vec![
            id.into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            p.installation_id.clone().into(),
            p.family_id.clone().into(),
        ];
        args.extend(ga);
        self.db.query(Statement::new(format!("SELECT b.* {} WHERE b.binding_id=? AND b.account_id=? AND b.assistant_id=? AND b.bridge_installation_id=? AND b.bridge_family_id=? AND {g}", Self::wake_from()), args)).await?.first().cloned().ok_or_else(|| Error::new("WAKE_PROOF_INVALID", "Wake proof is missing, expired or no longer valid."))
    }
    pub(super) async fn works_wake_dispatch(
        &self,
        p: &WorksPrincipal,
        operation: &str,
        v: Value,
    ) -> Result<Value> {
        if !p.messages_enabled {
            return Err(Error::forbidden());
        }
        match operation {
            "wake_create" => self.wake_create(p, v).await,
            "wake_confirm" => self.wake_confirm(p, v).await,
            "wake_status" => {
                fields(&v, &["binding_id"])?;
                let id = required(&v, "binding_id")?;
                check_id(&id)?;
                let row = self.wake_live(p, &id, false).await?;
                if optional_text(&row, "receiver_family_id").is_some() {
                    self.wake_live(p, &id, true).await?;
                }
                Ok(
                    json!({"binding_id":id,"status":if optional_text(&row,"receiver_family_id").is_some(){"verified"}else{"pending"},"registration_revision":integer(&row,"registration_revision")?,"expires_at":integer(&row,"expires_at")?}),
                )
            }
            "wake_poll" | "wake_check" => self.wake_poll(p, operation, v).await,
            _ => Err(Error::not_found()),
        }
    }
    async fn wake_create(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        fields(&v, &["session_id", "request_id", "target"])?;
        let session = required(&v, "session_id")?;
        check_id(&session)?;
        let req = required(&v, "request_id")?;
        check_request_id(&req)?;
        let target = required(&v, "target")?;
        if target.is_empty() || target.len() > 512 || target.chars().any(char::is_control) {
            return Err(Error::invalid("Invalid wake target."));
        }
        let now = self.clock.now_ms();
        let (bg, ba) = self.oauth_client_guard("f", now);
        let (identity, ia) = self.unified_identity_guard("account");
        let mut args: Vec<Sql> = vec![
            random_id("wake")?.into(),
            p.family_id.clone().into(),
            req.clone().into(),
            target.clone().into(),
            now.into(),
            (now + CHALLENGE_MS).into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            session.clone().into(),
            p.installation_id.clone().into(),
            p.family_id.clone().into(),
            now.into(),
            p.policy_revision.into(),
        ];
        args.extend(ba);
        args.extend(ia);
        let sql = format!(
            "INSERT INTO works_wake_bindings(binding_id,account_id,assistant_id,bridge_installation_id,bridge_family_id,bridge_attachment_id,execution_attachment_id,session_id,execution_installation_id,registration_revision,request_id,target,created_at,expires_at) SELECT ?,s.account_id,s.assistant_id,bi.installation_id,?,bm.attachment_id,em.attachment_id,s.session_id,s.installation_id,s.route_revision,?,?,?,? FROM works_sessions s JOIN works_assistants a ON a.account_id=s.account_id AND a.assistant_id=s.assistant_id JOIN accounts account ON account.account_id=s.account_id JOIN works_installations bi ON bi.account_id=s.account_id JOIN works_installations ei ON ei.account_id=s.account_id AND ei.installation_id=s.installation_id JOIN works_attachments bm ON bm.account_id=s.account_id AND bm.assistant_id=s.assistant_id AND bm.installation_id=bi.installation_id JOIN works_attachments em ON em.account_id=s.account_id AND em.assistant_id=s.assistant_id AND em.installation_id=s.installation_id JOIN works_families f ON f.family_id=? AND f.account_id=s.account_id AND f.installation_id=bi.installation_id WHERE s.account_id=? AND s.assistant_id=? AND s.session_id=? AND bi.installation_id=? AND f.family_id=? AND f.expires_at>? AND f.revoked_at IS NULL AND a.policy_revision=? AND a.status='active' AND a.messages_enabled=1 AND account.disabled=0 AND bi.status='active' AND ei.status='active' AND bi.installation_id<>ei.installation_id AND bm.status='active' AND em.status='active' AND s.status='registered' AND s.capability='wakeable' AND {bg} AND {identity} ON CONFLICT(bridge_family_id,request_id) DO NOTHING"
        );
        // f.family_id appears before WHERE placeholders.
        args.insert(6, p.family_id.clone().into());
        self.db.batch(vec![Statement::new(sql, args)]).await?;
        let rows = self
            .db
            .query(Statement::new(
                "SELECT * FROM works_wake_bindings WHERE bridge_family_id=? AND request_id=?",
                vec![p.family_id.clone().into(), req.into()],
            ))
            .await?;
        let row = rows.first().ok_or_else(|| {
            Error::new(
                "WAKE_PROOF_INVALID",
                "The session or either attachment is unavailable.",
            )
        })?;
        if text(row, "session_id")? != session || text(row, "target")? != target {
            return Err(Error::new(
                "IDEMPOTENCY_CONFLICT",
                "Request belongs to a different wake target.",
            ));
        }
        let id = text(row, "binding_id")?;
        self.wake_live(p, &id, false).await?;
        Ok(
            json!({"binding_id":id,"challenge":URL_SAFE_NO_PAD.encode(self.wake_mac(row)?.finalize().into_bytes()),"status":if optional_text(row,"receiver_family_id").is_some(){"verified"}else{"pending"},"registration_revision":integer(row,"registration_revision")?,"expires_at":integer(row,"expires_at")?}),
        )
    }
    async fn wake_confirm(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        fields(&v, &["binding_id", "challenge"])?;
        let id = required(&v, "binding_id")?;
        check_id(&id)?;
        let challenge = required(&v, "challenge")?;
        if challenge.len() > 128 {
            return Err(Error::invalid("Invalid wake challenge."));
        }
        let rows = self
            .db
            .query(Statement::new(
                "SELECT * FROM works_wake_bindings WHERE binding_id=?",
                vec![id.clone().into()],
            ))
            .await?;
        let row = rows.first().ok_or_else(Error::not_found)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(challenge)
            .map_err(|_| Error::invalid("Invalid wake challenge."))?;
        self.wake_mac(row)?
            .verify_slice(&bytes)
            .map_err(|_| Error::not_found())?;
        if text(row, "account_id")? != p.account_id
            || text(row, "assistant_id")? != p.assistant_id
            || text(row, "execution_installation_id")? != p.installation_id
        {
            return Err(Error::forbidden());
        }
        let now = self.clock.now_ms();
        let (g, ga) = self.wake_guard(false, now);
        let (rg, ra) = self.oauth_client_guard("receiver", now);
        let mut args: Vec<Sql> = vec![
            now.into(),
            (now + PROOF_MS).into(),
            p.family_id.clone().into(),
            id.clone().into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            p.installation_id.clone().into(),
            p.policy_revision.into(),
            p.family_id.clone().into(),
            now.into(),
        ];
        args.extend(ra);
        args.extend(ga);
        let sql = format!(
            "UPDATE works_wake_bindings SET confirmed_at=?,expires_at=?,receiver_family_id=? WHERE binding_id=? AND confirmed_at IS NULL AND EXISTS(SELECT 1 {} JOIN works_families receiver ON receiver.family_id=? AND receiver.account_id=b.account_id AND receiver.installation_id=b.execution_installation_id WHERE b.binding_id=works_wake_bindings.binding_id AND b.account_id=? AND b.assistant_id=? AND b.execution_installation_id=? AND a.policy_revision=? AND receiver.family_id=? AND receiver.expires_at>? AND receiver.revoked_at IS NULL AND {rg} AND {g}) RETURNING binding_id",
            Self::wake_from()
        );
        // The receiver join has an extra family parameter before the WHERE arguments.
        args.insert(4, p.family_id.clone().into());
        let changed = self.db.batch(vec![Statement::new(sql, args)]).await?;
        if changed[0].is_empty() {
            return Err(Error::new(
                "WAKE_PROOF_INVALID",
                "Challenge expired or has already been confirmed.",
            ));
        }
        Ok(json!({"binding_id":id,"status":"verified","expires_at":now+PROOF_MS}))
    }
    pub(super) async fn works_wake_account_status(&self, r: &Request) -> Result<Value> {
        let account = text(&self.account_session(r, false).await?, "account_id")?;
        let rows = self.db.query(Statement::new("SELECT b.*,i.label,a.name FROM works_wake_bindings b JOIN works_installations i ON i.account_id=b.account_id AND i.installation_id=b.bridge_installation_id JOIN works_assistants a ON a.account_id=b.account_id AND a.assistant_id=b.assistant_id WHERE b.account_id=? ORDER BY b.created_at DESC,b.binding_id", vec![account.into()])).await?;
        let mut bridges = Vec::new();
        for row in rows {
            let binding = text(&row, "binding_id")?;
            let (g, ga) = self.wake_guard(true, self.clock.now_ms());
            let mut args = vec![binding.clone().into()];
            args.extend(ga);
            let live = self
                .db
                .query(Statement::new(
                    format!(
                        "SELECT b.binding_id {} WHERE b.binding_id=? AND {g}",
                        Self::wake_from()
                    ),
                    args,
                ))
                .await?;
            let status = if !live.is_empty() {
                "verified"
            } else if integer(&row, "expires_at")? <= self.clock.now_ms() {
                "expired"
            } else if optional_text(&row, "receiver_family_id").is_none() {
                "pending"
            } else {
                "invalid"
            };
            let last_poll = match row.get("last_poll_at") {
                Some(Sql::Integer(n)) => Some(*n),
                _ => None,
            };
            bridges.push(json!({"binding_id":binding,"installation_id":text(&row,"bridge_installation_id")?,"label":text(&row,"label")?,"assistant_id":text(&row,"assistant_id")?,"assistant_name":text(&row,"name")?,"session_id":text(&row,"session_id")?,"status":status,"last_poll_at":last_poll,"proof_expires_at":integer(&row,"expires_at")?}));
        }
        Ok(json!({"bridges":bridges}))
    }

    async fn wake_poll(&self, p: &WorksPrincipal, op: &str, v: Value) -> Result<Value> {
        if op == "wake_poll" {
            fields(&v, &["binding_id", "cursor"])?;
        } else {
            fields(&v, &["binding_id", "delivery_id", "assignment_generation"])?;
        }
        let id = required(&v, "binding_id")?;
        check_id(&id)?;
        let (g, ga) = self.wake_guard(true, self.clock.now_ms());
        let mut args: Vec<Sql> = vec![
            id.clone().into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            p.installation_id.clone().into(),
            p.family_id.clone().into(),
        ];
        args.extend(ga);
        let filter = if op == "wake_check" {
            let delivery = required(&v, "delivery_id")?;
            check_id(&delivery)?;
            let generation = v
                .get("assignment_generation")
                .and_then(Value::as_i64)
                .filter(|n| *n > 0)
                .ok_or_else(|| Error::invalid("Missing generation."))?;
            args.push(delivery.into());
            args.push(generation.into());
            "AND d.delivery_id=? AND d.assignment_generation=?"
        } else {
            let cursor = match v.get("cursor") {
                None | Some(Value::Null) => "",
                Some(Value::String(s)) => {
                    check_id(s)?;
                    s
                }
                _ => return Err(Error::invalid("Invalid cursor.")),
            };
            args.push(cursor.into());
            "AND d.delivery_id>?"
        };
        let sql = format!(
            "SELECT d.delivery_id,d.message_id,d.assigned_session_id,d.assignment_generation {} JOIN works_deliveries d ON d.account_id=b.account_id AND d.assistant_id=b.assistant_id AND d.assigned_session_id=b.session_id WHERE b.binding_id=? AND b.account_id=? AND b.assistant_id=? AND b.bridge_installation_id=? AND b.bridge_family_id=? AND {g} AND d.state='assigned' {filter} ORDER BY d.delivery_id LIMIT 100",
            Self::wake_from()
        );
        self.wake_live(p, &id, true).await?;
        let rows = self.db.query(Statement::new(sql, args)).await?;
        if op == "wake_check" && rows.is_empty() {
            return Err(Error::new(
                "ASSIGNMENT_CHANGED",
                "Delivery is no longer assigned to this session.",
            ));
        }
        let now = self.clock.now_ms();
        let (guard, guard_args) = self.wake_guard(true, now);
        let mut stamp_args: Vec<Sql> = vec![
            now.into(),
            id.clone().into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            p.installation_id.clone().into(),
            p.family_id.clone().into(),
        ];
        stamp_args.extend(guard_args);
        let assignment_guard = if op == "wake_check" {
            stamp_args.push(required(&v, "delivery_id")?.into());
            stamp_args.push(
                v["assignment_generation"]
                    .as_i64()
                    .ok_or_else(|| Error::invalid("Missing generation."))?
                    .into(),
            );
            "AND EXISTS(SELECT 1 FROM works_deliveries d WHERE d.account_id=b.account_id AND d.assistant_id=b.assistant_id AND d.assigned_session_id=b.session_id AND d.delivery_id=? AND d.assignment_generation=? AND d.state='assigned')"
        } else {
            ""
        };
        let stamped=self.db.batch(vec![Statement::new(format!("UPDATE works_wake_bindings SET last_poll_at=? WHERE binding_id=? AND EXISTS(SELECT 1 {} WHERE b.binding_id=works_wake_bindings.binding_id AND b.account_id=? AND b.assistant_id=? AND b.bridge_installation_id=? AND b.bridge_family_id=? AND {guard} {assignment_guard}) RETURNING binding_id",Self::wake_from()),stamp_args)]).await?;
        if stamped[0].is_empty() {
            return Err(Error::new(
                "WAKE_PROOF_INVALID",
                "Wake proof changed during polling.",
            ));
        }
        if op == "wake_check" {
            if rows.is_empty() {
                return Err(Error::new(
                    "ASSIGNMENT_CHANGED",
                    "Delivery is no longer assigned to this session.",
                ));
            }
            return Ok(json!({"valid":true}));
        }
        let items=rows.iter().map(|r| -> Result<Value> {Ok(json!({"delivery_id":text(r,"delivery_id")?,"message_id":text(r,"message_id")?,"session_id":text(r,"assigned_session_id")?,"assignment_generation":integer(r,"assignment_generation")?}))}).collect::<Result<Vec<_>>>()?;
        let next_cursor = if rows.len() == 100 {
            Some(text(rows.last().unwrap(), "delivery_id")?)
        } else {
            None
        };
        Ok(json!({"items":items,"next_cursor":next_cursor}))
    }
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use crate::native::{FileStore, SqliteDb};
    use crate::service::{
        accounts_oidc::OidcConfig, oauth_client_registry::OAuthClientDiscoveryConfig,
    };

    fn principal(installation: &str, family: &str) -> WorksPrincipal {
        WorksPrincipal {
            token_digest: vec![],
            account_id: "acct".into(),
            assistant_id: "assistant".into(),
            installation_id: installation.into(),
            family_id: family.into(),
            policy_revision: 1,
            messages_enabled: true,
            keys_read: false,
            keys_write: false,
            keys_create: false,
            key_scope: "all".into(),
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn proof_binds_both_families_mounts_and_registration_and_fences_poll() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(
            SqliteDb::open(dir.path().join("db")).unwrap(),
            FileStore::open(dir.path().join("objects")).unwrap(),
            Config {
                admin_secret: "test-only-bridge-secret-more-than32".into(),
                public_origin: "https://example.test".into(),
            },
        );
        app.oidc = Some(OidcConfig {
            issuer: "issuer".into(),
            client_id: "client".into(),
            authorization_endpoint: "https://example.test/authorize".into(),
            token_endpoint: "https://example.test/token".into(),
            client_secret: None,
            keys: vec![],
            google_jwks: false,
        });
        app.oauth_discovery = Some(OAuthClientDiscoveryConfig::default());
        let now = app.clock.now_ms();
        let sqls = [
            "INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES ('acct','issuer','owner',0)",
            "INSERT INTO oauth_dynamic_clients(client_id,source,metadata_json,security_fingerprint,revision,valid_until,created_at,updated_at) VALUES('client','dcr','{}','fixture',1,9999999999999,0,0)",
            "INSERT INTO works_installations(installation_id,account_id,label,created_at) VALUES('bridge','acct','bridge',0),('host','acct','host',0),('other','acct','other',0)",
            "INSERT INTO works_assistants(assistant_id,account_id,name,messages_enabled,created_at) VALUES('assistant','acct','a',1,0)",
            "INSERT INTO works_attachments(attachment_id,account_id,assistant_id,installation_id,status,created_at) VALUES('bmount','acct','assistant','bridge','active',0),('hmount','acct','assistant','host','active',0),('omount','acct','assistant','other','active',0)",
            "INSERT INTO works_families(family_id,account_id,client_id,client_revision,resource,installation_id,created_at,expires_at) VALUES('bf','acct','client',1,'https://example.test/mcp','bridge',0,9999999999999),('hf','acct','client',1,'https://example.test/mcp','host',0,9999999999999),('of','acct','client',1,'https://example.test/mcp','other',0,9999999999999)",
            "INSERT INTO works_sessions(session_id,account_id,assistant_id,installation_id,host_kind,external_session_id,display_name,capability,created_at,last_seen_at) VALUES('session','acct','assistant','host','codex','thread','thread','wakeable',0,0)",
            "INSERT INTO works_messages(message_id,account_id,sender_assistant_id,conversation_id,idempotency_key,fingerprint,ciphertext,created_at) VALUES('message','acct','assistant','conversation','request','fp','body',0)",
            "INSERT INTO works_deliveries(delivery_id,account_id,message_id,assistant_id,conversation_id,target_mode,assigned_session_id,state,created_at,updated_at) VALUES('delivery','acct','message','assistant','conversation','session','session','assigned',0,0)",
        ];
        app.db
            .batch(
                sqls.into_iter()
                    .map(|sql| Statement::new(sql, vec![]))
                    .collect(),
            )
            .await
            .unwrap();
        let bridge = principal("bridge", "bf");
        let host = principal("host", "hf");
        let other = principal("other", "of");
        let created = app
            .works_wake_dispatch(
                &bridge,
                "wake_create",
                json!({"session_id":"session","request_id":"request","target":"thread_hash"}),
            )
            .await
            .unwrap();
        let id = created["binding_id"].as_str().unwrap();
        let confirm = json!({"binding_id":id,"challenge":created["challenge"]});
        assert!(
            app.works_wake_dispatch(&bridge, "wake_poll", json!({"binding_id":id}))
                .await
                .is_err()
        );
        assert!(
            app.works_wake_dispatch(&other, "wake_confirm", confirm.clone())
                .await
                .is_err()
        );
        app.works_wake_dispatch(&host, "wake_confirm", confirm)
            .await
            .unwrap();
        let poll = app
            .works_wake_dispatch(&bridge, "wake_poll", json!({"binding_id":id}))
            .await
            .unwrap();
        assert_eq!(poll["items"][0]["delivery_id"], "delivery");
        assert!(!poll.to_string().contains("body"));
        let stamp = app
            .db
            .query(Statement::new(
                "SELECT last_poll_at FROM works_wake_bindings WHERE binding_id=?",
                vec![id.into()],
            ))
            .await
            .unwrap();
        assert!(integer(&stamp[0], "last_poll_at").unwrap() >= now);
        app.db.batch(vec![Statement::new("UPDATE works_sessions SET route_revision=route_revision+1 WHERE session_id='session'",vec![])]).await.unwrap();
        assert!(
            app.works_wake_dispatch(&bridge, "wake_poll", json!({"binding_id":id}))
                .await
                .is_err()
        );
        app.db
            .batch(vec![Statement::new(
                "UPDATE works_sessions SET route_revision=1 WHERE session_id='session'",
                vec![],
            )])
            .await
            .unwrap();
        app.db
            .batch(vec![Statement::new(
                "UPDATE works_families SET revoked_at=? WHERE family_id='hf'",
                vec![now.into()],
            )])
            .await
            .unwrap();
        assert!(
            app.works_wake_dispatch(&bridge, "wake_poll", json!({"binding_id":id}))
                .await
                .is_err()
        );
        app.db
            .batch(vec![Statement::new(
                "UPDATE works_families SET revoked_at=NULL WHERE family_id='hf'",
                vec![],
            )])
            .await
            .unwrap();
        assert!(
            app.works_wake_dispatch(
                &bridge,
                "wake_check",
                json!({"binding_id":id,"delivery_id":"delivery","assignment_generation":2})
            )
            .await
            .is_err()
        );
        app.db.batch(vec![Statement::new("UPDATE works_attachments SET status='revoked',revoked_at=? WHERE attachment_id='hmount'",vec![now.into()])]).await.unwrap();
        assert!(
            app.works_wake_dispatch(&bridge, "wake_poll", json!({"binding_id":id}))
                .await
                .is_err()
        );
        app.db.batch(vec![Statement::new("INSERT INTO works_attachments(attachment_id,account_id,assistant_id,installation_id,status,created_at) VALUES('replacement','acct','assistant','host','active',?)",vec![now.into()])]).await.unwrap();
        assert!(
            app.works_wake_dispatch(&bridge, "wake_poll", json!({"binding_id":id}))
                .await
                .is_err()
        );
    }
}
