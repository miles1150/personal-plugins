//! Account-wide shared material snapshots, with immutable revisions and CAS.
use super::works_v2::WorksPrincipal;
use super::*;
use crate::domain::{check_id, check_request_id};
use crate::json::fields;
use base64::engine::general_purpose::STANDARD;
use std::collections::BTreeSet;

const MAX_ITEMS: usize = 64;
const MAX_SNAPSHOT_BYTES: usize = 128 * 1024;

fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::invalid(format!("{key} is required.")))
}

fn validate_items(items: &Value) -> Result<()> {
    let list = items
        .as_array()
        .filter(|a| a.len() <= MAX_ITEMS)
        .ok_or_else(|| Error::invalid("items must be an array of at most 64 items."))?;
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut total = 0usize;
    for item in list {
        fields(item, &["item_id", "path", "content_base64", "metadata"])?;
        let id = required(item, "item_id")?;
        check_id(id)?;
        let path = required(item, "path")?;
        if path.len() > 1024
            || path.contains(['\\', ':'])
            || path.chars().any(char::is_control)
            || path
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
            || !ids.insert(id)
            || !paths.insert(path)
        {
            return Err(Error::invalid(
                "Invalid or duplicate item ID or relative path.",
            ));
        }
        let encoded = item
            .get("content_base64")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::invalid("content_base64 must be a string."))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| Error::invalid("Invalid base64 content."))?;
        if STANDARD.encode(&bytes) != encoded {
            return Err(Error::invalid("Invalid base64 content."));
        }
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| Error::invalid("Snapshot too large."))?;
        if let Some(meta) = item.get("metadata")
            && !meta.is_object()
        {
            return Err(Error::invalid("metadata must be an object."));
        }
    }
    if total > MAX_SNAPSHOT_BYTES
        || serde_json::to_vec(items)
            .map_err(|_| Error::invalid("Invalid items."))?
            .len()
            > MAX_SNAPSHOT_BYTES * 2
    {
        return Err(Error::new(
            "RESOURCE_LIMIT",
            "Material snapshot is too large.",
        ));
    }
    Ok(())
}

impl<D: Database, O: ObjectStore> App<D, O> {
    pub(super) async fn works_materials_dispatch(
        &self,
        p: &WorksPrincipal,
        op: &str,
        value: Value,
    ) -> Result<Value> {
        if !p.messages_enabled {
            return Err(Error::forbidden());
        }
        if self.encryption.is_none() {
            return Err(Error::new(
                "ENCRYPTION_REQUIRED",
                "Shared materials require encrypted storage.",
            ));
        }
        match op {
            "materials_list" => self.works_materials_list(p).await,
            "materials_read" => self.works_materials_read(p, &value).await,
            "materials_publish" => self.works_materials_publish(p, &value).await,
            _ => Err(Error::not_found()),
        }
    }
    async fn works_materials_list(&self, p: &WorksPrincipal) -> Result<Value> {
        let (guard, guard_args) = self.works_v2_guard(p);
        let mut args = vec![p.account_id.clone().into()];
        args.extend(guard_args);
        let rows = self.db.query(Statement::new(format!("SELECT collection_id,MAX(revision) AS revision FROM works_material_revisions WHERE account_id=? AND {guard} GROUP BY collection_id ORDER BY collection_id LIMIT 501"), args)).await?;
        // An empty list must still distinguish an invalid credential.
        self.works_materials_live(p).await?;
        if rows.len() > 500 {
            return Err(Error::new("RESOURCE_LIMIT", "Too many collections."));
        }
        let values = rows.iter().map(|r| -> Result<Value> { Ok(json!({"collection_id":text(r,"collection_id")?,"revision":integer(r,"revision")?})) }).collect::<Result<Vec<_>>>()?;
        Ok(json!({"collections":values}))
    }
    async fn works_materials_live(&self, p: &WorksPrincipal) -> Result<()> {
        let (guard, args) = self.works_v2_guard(p);
        let rows = self
            .db
            .query(Statement::new(
                format!("SELECT 1 AS ok WHERE {guard}"),
                args,
            ))
            .await?;
        if rows.is_empty() {
            return Err(Error::unauthenticated());
        }
        Ok(())
    }
    async fn works_materials_read(&self, p: &WorksPrincipal, v: &Value) -> Result<Value> {
        fields(v, &["collection_id", "revision"])?;
        let id = required(v, "collection_id")?;
        check_id(id)?;
        let revision = match v.get("revision") {
            None => None,
            Some(v) => Some(
                v.as_i64()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| Error::invalid("Invalid revision."))?,
            ),
        };
        let (guard, guard_args) = self.works_v2_guard(p);
        let mut args = vec![
            p.account_id.clone().into(),
            id.into(),
            revision.into(),
            revision.into(),
        ];
        args.extend(guard_args);
        let rows = self.db.query(Statement::new(format!("SELECT revision,ciphertext FROM works_material_revisions WHERE account_id=? AND collection_id=? AND (? IS NULL OR revision=?) AND {guard} ORDER BY revision DESC LIMIT 1"), args)).await?;
        let row = match rows.first() {
            Some(r) => r,
            None => {
                self.works_materials_live(p).await?;
                return Err(Error::not_found());
            }
        };
        let rev = integer(row, "revision")?;
        let plain = self.open_field(
            &p.account_id,
            &format!("works/material/{id}/{rev}"),
            &text(row, "ciphertext")?,
        )?;
        let items: Value = serde_json::from_str(&plain)
            .map_err(|_| Error::internal("Invalid material snapshot."))?;
        self.works_materials_live(p).await?;
        Ok(json!({"collection_id":id,"revision":rev,"items":items}))
    }
    async fn works_materials_publish(&self, p: &WorksPrincipal, v: &Value) -> Result<Value> {
        fields(
            v,
            &["collection_id", "request_id", "expected_revision", "items"],
        )?;
        let id = required(v, "collection_id")?;
        let request = required(v, "request_id")?;
        check_id(id)?;
        check_request_id(request)?;
        let expected = v
            .get("expected_revision")
            .and_then(Value::as_i64)
            .filter(|n| *n >= 0 && *n < 9_007_199_254_740_991)
            .ok_or_else(|| Error::invalid("Invalid expected revision."))?;
        let items = v
            .get("items")
            .ok_or_else(|| Error::invalid("items is required."))?;
        validate_items(items)?;
        let fp = fingerprint(v)?;
        let (guard, guard_args) = self.works_v2_guard(p);
        let mut lookup_args = vec![
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            request.into(),
        ];
        lookup_args.extend(guard_args.clone());
        let lookup = || {
            Statement::new(
                format!(
                    "SELECT collection_id,revision,fingerprint FROM works_material_revisions WHERE account_id=? AND assistant_id=? AND request_id=? AND {guard}"
                ),
                lookup_args.clone(),
            )
        };
        let existing = self.db.query(lookup()).await?;
        if let Some(row) = existing.first() {
            return self.material_replay(p, request, row, &fp);
        }
        let next = expected + 1;
        let ciphertext = self.seal_field(
            &p.account_id,
            &format!("works/material/{id}/{next}"),
            &items.to_string(),
        )?;
        let mut args: Vec<Sql> = vec![
            p.account_id.clone().into(),
            id.into(),
            next.into(),
            p.assistant_id.clone().into(),
            request.into(),
            self.seal_field(
                &p.account_id,
                &format!("works/material/request/{}/{request}", p.assistant_id),
                &fp,
            )?
            .into(),
            ciphertext.into(),
            self.clock.now_ms().into(),
            p.account_id.clone().into(),
            id.into(),
            expected.into(),
            p.account_id.clone().into(),
            id.into(),
            p.account_id.clone().into(),
        ];
        args.extend(guard_args);
        let rows = self.db.batch(vec![Statement::new(format!("INSERT INTO works_material_revisions(account_id,collection_id,revision,assistant_id,request_id,fingerprint,ciphertext,created_at) SELECT ?,?,?,?,?,?,?,? WHERE (SELECT COALESCE(MAX(revision),0) FROM works_material_revisions WHERE account_id=? AND collection_id=?)=? AND (EXISTS(SELECT 1 FROM works_material_revisions WHERE account_id=? AND collection_id=?) OR (SELECT COUNT(DISTINCT collection_id) FROM works_material_revisions WHERE account_id=?)<500) AND {guard} ON CONFLICT DO NOTHING RETURNING revision"),args)]).await?;
        if !rows[0].is_empty() {
            return Ok(json!({"collection_id":id,"revision":next,"published":true}));
        }
        let existing = self.db.query(lookup()).await?;
        if let Some(row) = existing.first() {
            return self.material_replay(p, request, row, &fp);
        }
        self.works_materials_live(p).await?;
        Err(Error::new(
            "REVISION_CONFLICT",
            "Material revision changed or collection limit reached.",
        ))
    }

    fn material_replay(
        &self,
        p: &WorksPrincipal,
        request: &str,
        row: &Row,
        fp: &str,
    ) -> Result<Value> {
        if self.open_field(
            &p.account_id,
            &format!("works/material/request/{}/{request}", p.assistant_id),
            &text(row, "fingerprint")?,
        )? != fp
        {
            return Err(Error::new(
                "IDEMPOTENCY_CONFLICT",
                "Request ID was already used with different material.",
            ));
        }
        Ok(
            json!({"collection_id":text(row,"collection_id")?,"revision":integer(row,"revision")?,"published":true}),
        )
    }
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use crate::native::{FileStore, SqliteDb};

    #[tokio::test]
    async fn account_shared_snapshots_preserve_bytes_and_fence_writes() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(
            SqliteDb::open(dir.path().join("db")).unwrap(),
            FileStore::open(dir.path().join("objects")).unwrap(),
            Config {
                admin_secret: "test-secret".into(),
                public_origin: "http://localhost".into(),
            },
        )
        .with_encryption_keys(
            &format!(
                r#"{{"active":"test","keys":{{"test":"{}"}}}}"#,
                "11".repeat(32)
            ),
            false,
        )
        .unwrap();
        app.oidc = Some(accounts_oidc::OidcConfig {
            issuer: "i".into(),
            client_id: "test".into(),
            authorization_endpoint: "https://example.test/auth".into(),
            token_endpoint: "https://example.test/token".into(),
            client_secret: None,
            keys: vec![],
            google_jwks: false,
        });
        app.oauth_discovery = Some(oauth_client_registry::OAuthClientDiscoveryConfig::default());
        app.db.batch(vec![
            Statement::new("INSERT INTO oauth_dynamic_clients(client_id,source,metadata_json,security_fingerprint,revision,valid_until,created_at,updated_at) VALUES ('client','dcr','{}','fingerprint',1,9999999999999,0,0)",vec![]),
            Statement::new("INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES ('a','i','s',0),('b','i','other',0)",vec![]),
            Statement::new("INSERT INTO works_installations(installation_id,account_id,label,created_at) VALUES ('inst','a','device',0),('other','b','device',0)",vec![]),
            Statement::new("INSERT INTO works_families(family_id,account_id,client_id,client_revision,resource,installation_id,created_at,expires_at) VALUES ('family','a','client',1,'http://localhost/mcp','inst',0,9999999999999),('otherfamily','b','client',1,'http://localhost/mcp','other',0,9999999999999)",vec![]),
            Statement::new("INSERT INTO works_access_tokens(token_digest,family_id,expires_at) VALUES (?,'family',9999999999999),(?,'otherfamily',9999999999999)",vec![vec![7u8;32].into(),vec![8u8;32].into()]),
            Statement::new("INSERT INTO works_assistants(assistant_id,account_id,name,messages_enabled,created_at) VALUES ('writer','a','writer',1,0),('reader','a','reader',1,0),('outside','b','outside',1,0)",vec![]),
            Statement::new("INSERT INTO works_attachments(attachment_id,account_id,assistant_id,installation_id,status,created_at) VALUES ('w','a','writer','inst','active',0),('r','a','reader','inst','active',0),('o','b','outside','other','active',0)",vec![]),
        ]).await.unwrap();
        let principal = |id: &str, account: &str, installation: &str, family: &str, digest: u8| {
            WorksPrincipal {
                account_id: account.into(),
                assistant_id: id.into(),
                installation_id: installation.into(),
                family_id: family.into(),
                token_digest: vec![digest; 32],
                policy_revision: 1,
                messages_enabled: true,
                keys_read: false,
                keys_write: false,
                keys_create: false,
                key_scope: "all".into(),
            }
        };
        let writer = principal("writer", "a", "inst", "family", 7);
        let reader = principal("reader", "a", "inst", "family", 7);
        let outside = principal("outside", "b", "other", "otherfamily", 8);
        let item = json!({"item_id":"file1","path":"folder/private.txt","content_base64":STANDARD.encode(b"secret \0 data"),"metadata":{"source":"private metadata"}});
        let publish = json!({"collection_id":"shared","request_id":"req1","expected_revision":0,"items":[item]});
        let result = app
            .works_materials_dispatch(&writer, "materials_publish", publish.clone())
            .await
            .unwrap();
        assert_eq!(result["revision"], 1);
        assert_eq!(
            app.works_materials_dispatch(&writer, "materials_publish", publish.clone())
                .await
                .unwrap(),
            result
        );
        let mut bad = publish.clone();
        bad["items"] = json!([]);
        assert_eq!(
            app.works_materials_dispatch(&writer, "materials_publish", bad)
                .await
                .unwrap_err()
                .code,
            "IDEMPOTENCY_CONFLICT"
        );
        let cipher = app
            .db
            .query(Statement::new(
                "SELECT ciphertext FROM works_material_revisions",
                vec![],
            ))
            .await
            .unwrap();
        let sealed = text(&cipher[0], "ciphertext").unwrap();
        assert!(sealed.starts_with("wpenc1:"));
        assert!(!sealed.contains("private"));
        let read = app
            .works_materials_dispatch(&reader, "materials_read", json!({"collection_id":"shared"}))
            .await
            .unwrap();
        assert_eq!(read["items"], publish["items"]);
        assert!(
            app.works_materials_dispatch(
                &outside,
                "materials_read",
                json!({"collection_id":"shared"})
            )
            .await
            .is_err()
        );
        let update =
            json!({"collection_id":"shared","request_id":"req2","expected_revision":1,"items":[]});
        assert_eq!(
            app.works_materials_dispatch(&reader, "materials_publish", update.clone())
                .await
                .unwrap()["revision"],
            2
        );
        let stale =
            json!({"collection_id":"shared","request_id":"req3","expected_revision":1,"items":[]});
        assert_eq!(
            app.works_materials_dispatch(&writer, "materials_publish", stale)
                .await
                .unwrap_err()
                .code,
            "REVISION_CONFLICT"
        );
        assert_eq!(
            app.works_materials_dispatch(
                &reader,
                "materials_read",
                json!({"collection_id":"shared","revision":1})
            )
            .await
            .unwrap()["items"],
            publish["items"]
        );
        app.db
            .batch(vec![Statement::new(
                "UPDATE works_attachments SET status='revoked' WHERE attachment_id='r'",
                vec![],
            )])
            .await
            .unwrap();
        assert!(
            app.works_materials_dispatch(
                &reader,
                "materials_read",
                json!({"collection_id":"shared"})
            )
            .await
            .is_err()
        );
        assert!(app.works_materials_dispatch(&reader,"materials_publish",json!({"collection_id":"shared","request_id":"req4","expected_revision":2,"items":[]})).await.is_err());
        app.db.batch(vec![Statement::new("UPDATE works_assistants SET messages_enabled=0,policy_revision=policy_revision+1 WHERE assistant_id='writer'",vec![])]).await.unwrap();
        assert!(
            app.works_materials_dispatch(&writer, "materials_list", json!({}))
                .await
                .is_err()
        );
    }
}
