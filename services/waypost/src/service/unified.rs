//! One public OAuth resource with independent message and key business grants.
//! `db` is the account/message authority; `keys_db` is a separate storage handle.
//! This is a storage/authorization boundary, not a separate process boundary.
//! Cross-database revocation is deliberately not described as atomic: a mutation
//! authorized before revocation can finish. Every new call checks the parent.
use super::keys::tools::{KeysPrincipal, KeysTools};
use super::*;
use crate::gateway::mcp::{ServerInfo, ToolCatalog, ToolSpec, schema};
use crate::json::{fields, required};

pub(super) struct UnifiedTools<'a, D, O>(pub(super) &'a App<D, O>);
pub(super) struct UnifiedPrincipal {
    token: String,
}
impl<D: Database, O: ObjectStore> ToolCatalog for UnifiedTools<'_, D, O> {
    type Principal = UnifiedPrincipal;
    fn server(&self) -> ServerInfo {
        ServerInfo {
            name: "miles-works",
            instructions: "Use works_identity to inspect the current connection, then waypost_describe for messaging. Message and secret permissions are independent. fetch_entry returns secret values to the connected host; save values to private files and never repeat them in replies or logs. Persist stable request IDs before writes and inbox contents before acknowledging.",
        }
    }
    fn tools(&self) -> Vec<ToolSpec> {
        let mut tools = super::mcp::WaypostTools(self.0).tools();
        tools.extend(KeysTools(self.0).tools());
        tools.push(ToolSpec{name:"works_identity",description:"Inspect the authenticated account, connection and independently granted capabilities. Never returns credentials or secret values.",input_schema:schema(json!({}),&[]),read_only:true});
        tools
    }
    async fn authenticate(&self, r: &Request) -> Result<UnifiedPrincipal> {
        let token = bearer(r)?;
        self.0
            .unified_parent(&Sha256::digest(token.as_bytes()))
            .await?;
        Ok(UnifiedPrincipal { token })
    }
    async fn call(&self, p: &UnifiedPrincipal, name: &str, args: Value) -> Result<Value> {
        let digest = Sha256::digest(p.token.as_bytes()).to_vec();
        let parent = self.0.unified_parent(&digest).await?;
        if name == "works_identity" {
            fields(&args, &[])?;
            return Ok(
                json!({"account_id":text(&parent,"account_id")?,"connection_id":text(&parent,"family_id")?,"space_id":text(&parent,"space_id")?,"actor_id":text(&parent,"actor_id")?,"consumer_id":text(&parent,"consumer_id")?,"scope":text(&parent,"scope")?,"messages_enabled":integer(&parent,"messages_enabled")?==1,"keys_enabled":integer(&parent,"keys_enabled")?==1}),
            );
        }
        if name.starts_with("waypost_") {
            if integer(&parent, "messages_enabled")? != 1 {
                return Err(Error::forbidden());
            }
            let request = Request::json("POST", "/mcp", &p.token, json!({}));
            let principal = self.0.authenticate(&request).await?;
            return self.0.call_tool(&principal, name, args).await;
        }
        if !KeysTools(self.0).tools().iter().any(|t| t.name == name) {
            return Err(Error::not_found());
        }
        if integer(&parent, "keys_enabled")? != 1 {
            return Err(Error::forbidden());
        }
        let principal = self.0.unified_keys_principal(&parent, digest).await?;
        KeysTools(self.0).call(&principal, name, args).await
    }
}
impl<D: Database, O: ObjectStore> App<D, O> {
    /// Fresh unified deployment only. The caller supplies a separate keys DB.
    /// Existing vaults are not silently imported: account IDs are part of AAD.
    pub fn with_unified_keys(mut self, db: D, keyring: &str) -> Result<Self> {
        if self.oidc.is_none()
            || self.operator_enabled
            || !self
                .oauth_discovery
                .as_ref()
                .is_some_and(|c| c.dynamic_registration)
        {
            return Err(Error::invalid(
                "Unified mode requires account sign-in, dynamic registration and disabled operator access.",
            ));
        }
        self.keys = Some(keys::KeysConfig {
            keyring: keys::crypto::Keyring::parse(keyring)?,
        });
        self.keys_db = Some(db);
        self.limits.max_json_bytes = keys::MAX_JSON_BYTES;
        Ok(self)
    }
    /// Personal production deployment: no fixture/pinned-key provider and no
    /// first-signer-wins account initialization. Native fixtures use the regular
    /// builder and do not enable this production policy.
    pub(crate) fn validate_unified_production(&self) -> Result<()> {
        let oidc = self
            .oidc
            .as_ref()
            .ok_or_else(|| Error::invalid("Production unified mode requires Google sign-in."))?;
        oidc.validate()?;
        if !oidc.google_jwks
            || oidc.client_secret.as_ref().is_none_or(|s| s.is_empty())
            || self.account_owner_subject.is_none()
            || self.keys_db.is_none()
            || self.keys.is_none()
            || self.encryption.is_none()
            || self.operator_enabled
        {
            return Err(Error::invalid(
                "Production unified mode requires rotating Google verification, an explicit owner subject, separate key storage, encrypted messages and disabled operator access.",
            ));
        }
        self.validate_runtime_auth()
    }
    /// Guard all unified credential uses against the currently configured IdP
    /// and optional owner pin, including already-issued access/refresh tokens.
    pub(super) fn unified_identity_guard(&self, alias: &str) -> (String, Vec<Sql>) {
        let Some(oidc) = self.oidc.as_ref() else {
            return ("0".into(), vec![]);
        };
        let mut args = vec![oidc.issuer.clone().into()];
        let mut guard = format!("{alias}.issuer=?");
        if let Some(subject) = self.account_owner_subject.as_ref() {
            guard.push_str(&format!(" AND {alias}.subject=?"));
            args.push(subject.clone().into());
        }
        (guard, args)
    }
    pub(super) fn keys_store(&self) -> &D {
        self.keys_db.as_ref().unwrap_or(&self.db)
    }
    pub(super) fn keys_acl_table(&self, p: &KeysPrincipal) -> &'static str {
        if p.unified.is_some() {
            "unified_key_consent_entries"
        } else {
            "key_consent_entries"
        }
    }
    /// Parent authorization is always loaded from the control database; neither
    /// caller-supplied identity headers nor local key rows establish authority.
    pub(super) async fn unified_parent(&self, digest: &[u8]) -> Result<Row> {
        let now = self.clock.now_ms();
        let (guard, client_args) = self.oauth_client_guard("f", now);
        let mut args = vec![
            digest.to_vec().into(),
            now.into(),
            now.into(),
            self.oauth_resource().into(),
        ];
        let (identity_guard, identity_args) = self.unified_identity_guard("a");
        args.extend(identity_args);
        args.extend(client_args);
        let rows=self.db.query(Statement::new(format!("SELECT f.family_id,f.account_id,f.client_id,f.scope,f.space_id,f.actor_id,f.consumer_id,a.issuer,a.subject,q.request_id,g.messages_enabled,g.keys_enabled,g.key_version FROM tokens t JOIN oauth_access_tokens oat ON oat.token_id=t.token_id JOIN oauth_families f ON f.family_id=oat.family_id JOIN oauth_requests q ON q.family_id=f.family_id JOIN unified_connection_grants g ON g.request_id=q.request_id AND g.account_id=f.account_id JOIN accounts a ON a.account_id=f.account_id WHERE t.token_digest=? AND t.credential_kind='oauth' AND t.revoked_at IS NULL AND t.expires_at>? AND f.revoked_at IS NULL AND f.expires_at>? AND f.resource=? AND a.disabled=0 AND q.status='consumed' AND {identity_guard} AND {guard}"),args)).await?;
        rows.first().cloned().ok_or_else(Error::unauthenticated)
    }
    async fn unified_keys_principal(&self, parent: &Row, digest: Vec<u8>) -> Result<KeysPrincipal> {
        let request = text(parent, "request_id")?;
        let rows=self.keys_store().query(Statement::new("SELECT g.* FROM unified_key_grants g JOIN accounts a ON a.account_id=g.account_id WHERE g.request_id=? AND g.version=? AND g.revoked_at IS NULL AND a.issuer=? AND a.subject=? AND a.disabled=0",vec![request.clone().into(),integer(parent,"key_version")?.into(),text(parent,"issuer")?.into(),text(parent,"subject")?.into()])).await?;
        let row = rows.first().ok_or_else(Error::unauthenticated)?;
        let scope = text(parent, "scope")?;
        let scopes: Vec<_> = scope.split_whitespace().collect();
        // The local scope must be the exact consent snapshot too.
        if text(row, "scope")? != scope {
            return Err(Error::unauthenticated());
        }
        Ok(KeysPrincipal {
            unified: Some((digest, integer(row, "version")?)),
            token_digest: vec![],
            family_id: text(parent, "family_id")?,
            account_id: text(row, "account_id")?,
            client_id: text(parent, "client_id")?,
            request_id: request,
            read: scopes.contains(&"keys:read"),
            write: scopes.contains(&"keys:write"),
            can_create: integer(row, "can_create")? == 1,
            all_read: integer(row, "all_read")? == 1,
            all_write: integer(row, "all_write")? == 1,
        })
    }
    /// Stable identity in the separate store is keyed only by the verified OIDC
    /// issuer and subject. Emails and matching display names are never joined.
    async fn unified_key_account(&self, account: &str) -> Result<String> {
        let rows = self
            .db
            .query(Statement::new(
                "SELECT issuer,subject FROM accounts WHERE account_id=? AND disabled=0",
                vec![account.into()],
            ))
            .await?;
        let row = rows.first().ok_or_else(Error::unauthenticated)?;
        let issuer = text(row, "issuer")?;
        let subject = text(row, "subject")?;
        let rows=self.keys_store().batch(vec![Statement::new("INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES(?,?,?,?) ON CONFLICT(issuer,subject) DO NOTHING",vec![random_id("account")?.into(),issuer.clone().into(),subject.clone().into(),self.clock.now_ms().into()]),Statement::new("SELECT account_id FROM accounts WHERE issuer=? AND subject=? AND disabled=0",vec![issuer.into(),subject.into()])]).await?;
        text(
            rows[1].first().ok_or_else(Error::unauthenticated)?,
            "account_id",
        )
    }
    pub(super) async fn unified_consent_view(&self, session: &Row) -> Result<Value> {
        let account = self
            .unified_key_account(&text(session, "account_id")?)
            .await?;
        let mut mapped = session.clone();
        mapped.insert("account_id".into(), account.into());
        self.keys_consent_view(&mapped).await
    }
    /// Store a local grant before approving the parent request. An orphan local
    /// grant has no authority because no live parent family can reference it.
    pub(super) async fn unified_prepare_consent(
        &self,
        id: &str,
        session: &Row,
        body: &Value,
    ) -> Result<bool> {
        let invalid = || oauth::oauth_error("invalid_request");
        let rows=self.db.query(Statement::new("SELECT scope FROM oauth_requests WHERE request_id=? AND status='pending' AND kind='code'",vec![id.into()])).await?;
        let requested = text(rows.first().ok_or_else(invalid)?, "scope")?;
        let scopes: Vec<_> = requested.split_whitespace().collect();
        let flag = |key: &str| -> Result<i64> {
            match body.get(key) {
                None => Ok(0),
                Some(Value::Bool(v)) => Ok(i64::from(*v)),
                _ => Err(invalid()),
            }
        };
        let create = flag("can_create")?;
        let read = flag("all_read")?;
        let write = flag("all_write")?;
        let mut selected = vec![];
        let mut wants_read = read == 1;
        let mut wants_write = create == 1 || write == 1;
        if let Some(entries) = body.get("entries") {
            let entries = entries
                .as_array()
                .filter(|e| e.len() <= keys::MAX_ENTRIES as usize)
                .ok_or_else(invalid)?;
            let mut names = std::collections::BTreeSet::new();
            for entry in entries {
                fields(entry, &["name", "read", "write"])?;
                let name = required(entry, "name")?;
                keys::tools::check_name(&name)?;
                if !names.insert(name.clone()) {
                    return Err(invalid());
                }
                let bit = |key: &str| -> Result<bool> {
                    match entry.get(key) {
                        None => Ok(false),
                        Some(Value::Bool(v)) => Ok(*v),
                        _ => Err(invalid()),
                    }
                };
                let r = bit("read")?;
                let w = bit("write")?;
                wants_read |= r;
                wants_write |= w;
                if r || w {
                    selected.push(json!({"name":name,"read":i64::from(r),"write":i64::from(w)}));
                }
            }
        }
        if (wants_read && !scopes.contains(&"keys:read"))
            || (wants_write && !scopes.contains(&"keys:write"))
        {
            return Err(oauth::oauth_error("invalid_scope"));
        }
        let enabled = wants_read || wants_write;
        // Consent can narrow a request; refresh never increases it again.
        let scope = [
            Some("waypost"),
            wants_read.then_some("keys:read"),
            wants_write.then_some("keys:write"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
        if enabled {
            let account = self
                .unified_key_account(&text(session, "account_id")?)
                .await?;
            selected.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
            let consent_digest = fingerprint(
                &json!({"account_id":account,"scope":scope,"can_create":create,"all_read":read,"all_write":write,"entries":selected}),
            )?;
            let rows=self.keys_store().batch(vec![
                Statement::new("INSERT INTO unified_key_grants(request_id,account_id,scope,consent_digest,version,can_create,all_read,all_write,created_at) VALUES(?,?,?,?,1,?,?,?,?) ON CONFLICT(request_id) DO NOTHING",vec![id.into(),account.clone().into(),scope.clone().into(),consent_digest.clone().into(),create.into(),read.into(),write.into(),self.clock.now_ms().into()]),
                Statement::new("INSERT INTO unified_key_consent_entries(request_id,entry_id,can_read,can_write) SELECT ?,e.entry_id,json_extract(j.value,'$.read'),json_extract(j.value,'$.write') FROM json_each(?) j JOIN key_entries e ON e.name=json_extract(j.value,'$.name') AND e.account_id=? WHERE EXISTS(SELECT 1 FROM unified_key_grants WHERE request_id=? AND consent_digest=?) ON CONFLICT(request_id,entry_id) DO NOTHING",vec![id.into(),Value::Array(selected.clone()).to_string().into(),account.into(),id.into(),consent_digest.clone().into()]),
                Statement::new("SELECT consent_digest,(SELECT count(*) FROM unified_key_consent_entries c WHERE c.request_id=g.request_id) AS entries FROM unified_key_grants g WHERE request_id=?",vec![id.into()]),
            ]).await?;
            let installed = rows[2].first().ok_or_else(invalid)?;
            if text(installed, "consent_digest")? != consent_digest
                || integer(installed, "entries")? != selected.len() as i64
            {
                return Err(invalid());
            }
        }
        self.db
            .batch(vec![Statement::new(
                "UPDATE oauth_requests SET scope=? WHERE request_id=? AND status='pending'",
                vec![scope.into(), id.into()],
            )])
            .await?;
        Ok(enabled)
    }
    pub(super) async fn unified_keys_account_handle(&self, r: &Request) -> Response {
        let result = self.unified_keys_account_action(r).await;
        match result {
            Ok(v) => Response::json(200, json!({"ok":true,"data":v})),
            Err(e) => Response::json(e.status(), json!({"ok":false,"error":e.value()})),
        }
    }
    async fn unified_keys_account_action(&self, r: &Request) -> Result<Value> {
        let session = self.account_session(r, r.method != "GET").await?;
        let account = text(&session, "account_id")?;
        let now = self.clock.now_ms();
        if r.method != "GET" && integer(&session, "created_at")? < now - 900_000 {
            return Err(Error::new(
                "REAUTHENTICATION_REQUIRED",
                "Sign in again before changing connection permissions.",
            ));
        }
        match (r.method.as_str(), r.path.as_str()) {
            ("GET", "/account/keys/grants") => {
                let rows=self.db.query(Statement::new("SELECT f.family_id AS grant_id,f.client_id,f.scope,f.created_at,q.request_id,g.key_version FROM oauth_families f JOIN oauth_requests q ON q.family_id=f.family_id JOIN unified_connection_grants g ON g.request_id=q.request_id WHERE f.account_id=? AND f.revoked_at IS NULL AND f.expires_at>? AND g.keys_enabled=1 ORDER BY f.created_at DESC LIMIT 100",vec![account.into(),now.into()])).await?;
                let ids = rows
                    .iter()
                    .map(|r| text(r, "request_id"))
                    .collect::<Result<Vec<_>>>()?;
                let local=self.keys_store().query(Statement::new("SELECT g.request_id,g.version,g.can_create,g.all_read,g.all_write,(SELECT json_group_array(json_object('name',e.name,'read',c.can_read,'write',c.can_write)) FROM unified_key_consent_entries c JOIN key_entries e ON e.entry_id=c.entry_id WHERE c.request_id=g.request_id) AS entries FROM unified_key_grants g WHERE request_id IN(SELECT value FROM json_each(?)) AND revoked_at IS NULL",vec![json!(ids).to_string().into()])).await?;
                let mut grants = vec![];
                for row in rows {
                    let request = text(&row, "request_id")?;
                    if let Some(k) = local
                        .iter()
                        .find(|k| matches!(k.get("request_id"),Some(Sql::Text(id)) if id==&request))
                    {
                        if integer(k, "version")? != integer(&row, "key_version")? {
                            continue;
                        }
                        grants.push(json!({"grant_id":text(&row,"grant_id")?,"client_id":text(&row,"client_id")?,"scope":text(&row,"scope")?,"created_at":stamp(integer(&row,"created_at")?)?,"can_create":integer(k,"can_create")?==1,"all_read":integer(k,"all_read")?==1,"all_write":integer(k,"all_write")?==1,"entries":serde_json::from_str::<Value>(&text(k,"entries")?).map_err(|_|Error::storage())?}));
                    }
                }
                Ok(json!({"grants":grants}))
            }
            ("POST", "/account/keys/grants/revoke" | "/account/connections/grants/revoke") => {
                let body = crate::json::parse(&r.body, 4096)?;
                let messages = r.path == "/account/connections/grants/revoke";
                fields(
                    &body,
                    if messages {
                        &["grant_id", "capability"]
                    } else {
                        &["grant_id"]
                    },
                )?;
                let capability = if messages {
                    required(&body, "capability")?
                } else {
                    "keys".into()
                };
                if !["messages", "keys"].contains(&capability.as_str()) {
                    return Err(Error::invalid("Unknown connection capability."));
                }
                let id = required(&body, "grant_id")?;
                // Central first: failed local cleanup cannot restore authority.
                let cookie = accounts::account_cookie(r, &self.config.public_origin)?;
                let reduction = if capability == "messages" {
                    "messages_enabled=0"
                } else {
                    "keys_enabled=0,key_version=key_version+1"
                };
                let rows=self.db.batch(vec![Statement::new(format!("UPDATE unified_connection_grants SET {reduction} WHERE account_id=? AND request_id IN(SELECT request_id FROM oauth_requests WHERE family_id=?) AND EXISTS(SELECT 1 FROM account_sessions s JOIN accounts a ON a.account_id=s.account_id WHERE s.session_digest=? AND s.account_id=unified_connection_grants.account_id AND s.expires_at>? AND s.created_at>=? AND a.disabled=0) RETURNING request_id"),vec![account.into(),id.into(),Sha256::digest(cookie.as_bytes()).to_vec().into(),now.into(),(now-900_000).into()])]).await?;
                let row = rows[0].first().ok_or_else(Error::not_found)?;
                if capability == "keys" {
                    self.keys_store().batch(vec![Statement::new("UPDATE unified_key_grants SET revoked_at=COALESCE(revoked_at,?),version=version+1 WHERE request_id=?",vec![now.into(),text(row,"request_id")?.into()])]).await?;
                }
                Ok(json!({"revoked":true,"capability":capability}))
            }
            _ => Err(Error::not_found()),
        }
    }
}
