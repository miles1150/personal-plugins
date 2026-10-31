//! Bootstrap credentials have no legacy actor or business permission.
use super::*;
use crate::storage::driver::optional_text;
#[derive(Clone, Debug)]
pub(super) struct WorksPrincipal {
    pub(super) account_id: String,
    pub(super) assistant_id: String,
    pub(super) installation_id: String,
    pub(super) family_id: String,
    pub(super) token_digest: Vec<u8>,
    pub(super) policy_revision: i64,
    pub(super) messages_enabled: bool,
    pub(super) keys_read: bool,
    pub(super) keys_write: bool,
    pub(super) keys_create: bool,
    pub(super) key_scope: String,
}
impl<D: Database, O: ObjectStore> App<D, O> {
    pub(super) async fn works_v2_bootstrap(&self, r: &Request) -> Result<Row> {
        let token = bearer(r)?;
        let now = self.clock.now_ms();
        let (guard, client_args) = self.oauth_client_guard("f", now);
        let (identity, identity_args) = self.unified_identity_guard("a");
        let mut args: Vec<Sql> = vec![
            Sha256::digest(token.as_bytes()).to_vec().into(),
            now.into(),
            now.into(),
            self.oauth_resource().into(),
        ];
        args.extend(identity_args);
        args.extend(client_args);
        let rows=self.db.query(Statement::new(format!("SELECT f.family_id,f.account_id,i.installation_id,i.label FROM works_access_tokens t JOIN works_families f ON f.family_id=t.family_id JOIN accounts a ON a.account_id=f.account_id JOIN works_installations i ON i.installation_id=f.installation_id AND i.account_id=f.account_id WHERE t.token_digest=? AND t.expires_at>? AND f.expires_at>? AND f.resource=? AND f.revoked_at IS NULL AND a.disabled=0 AND i.status='active' AND {identity} AND {guard}"),args)).await?;
        rows.first().cloned().ok_or_else(Error::unauthenticated)
    }
    pub(super) async fn works_v2_authorize(
        &self,
        r: &Request,
        assistant_id: &str,
    ) -> Result<WorksPrincipal> {
        let b = self.works_v2_bootstrap(r).await?;
        let account_id = text(&b, "account_id")?;
        let installation_id = text(&b, "installation_id")?;
        let family_id = text(&b, "family_id")?;
        let rows=self.db.query(Statement::new("SELECT a.* FROM works_assistants a JOIN works_attachments m ON m.assistant_id=a.assistant_id AND m.account_id=a.account_id WHERE a.assistant_id=? AND a.account_id=? AND a.status='active' AND m.installation_id=? AND m.status='active'",vec![assistant_id.into(),account_id.clone().into(),installation_id.clone().into()])).await?;
        let a = rows.first().ok_or_else(|| {
            Error::new(
                "ASSISTANT_REQUIRED",
                "An active assistant attachment is required.",
            )
        })?;
        Ok(WorksPrincipal {
            account_id,
            assistant_id: assistant_id.into(),
            installation_id,
            family_id,
            token_digest: Sha256::digest(bearer(r)?.as_bytes()).to_vec(),
            policy_revision: integer(a, "policy_revision")?,
            messages_enabled: integer(a, "messages_enabled")? == 1,
            keys_read: integer(a, "keys_read")? == 1,
            keys_write: integer(a, "keys_write")? == 1,
            keys_create: integer(a, "keys_create")? == 1,
            key_scope: text(a, "key_scope")?,
        })
    }
    /// Repeat full credential and policy checks in the final control-DB SQL statement.
    pub(super) fn works_v2_guard(&self, p: &WorksPrincipal) -> (String, Vec<Sql>) {
        let now = self.clock.now_ms();
        let (client, client_args) = self.oauth_client_guard("f", now);
        let (identity, identity_args) = self.unified_identity_guard("ac");
        let mut args: Vec<Sql> = vec![
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            p.installation_id.clone().into(),
            p.family_id.clone().into(),
            p.token_digest.clone().into(),
            now.into(),
            now.into(),
            self.oauth_resource().into(),
            p.policy_revision.into(),
        ];
        args.extend(identity_args);
        args.extend(client_args);
        (
            format!(
                "EXISTS(SELECT 1 FROM works_assistants a JOIN works_installations i ON i.account_id=a.account_id JOIN works_attachments m ON m.account_id=a.account_id AND m.assistant_id=a.assistant_id AND m.installation_id=i.installation_id JOIN works_families f ON f.account_id=i.account_id AND f.installation_id=i.installation_id JOIN works_access_tokens t ON t.family_id=f.family_id JOIN accounts ac ON ac.account_id=f.account_id WHERE a.account_id=? AND a.assistant_id=? AND i.installation_id=? AND f.family_id=? AND t.token_digest=? AND t.expires_at>? AND f.expires_at>? AND f.resource=? AND a.policy_revision=? AND f.revoked_at IS NULL AND a.status='active' AND a.messages_enabled=1 AND i.status='active' AND m.status='active' AND ac.disabled=0 AND {identity} AND {client})"
            ),
            args,
        )
    }
    /// Bootstrap can inspect only its own installation and submit an attachment request.
    pub(super) async fn works_v2_bootstrap_action(
        &self,
        r: &Request,
        operation: &str,
        input: Value,
    ) -> Result<Value> {
        let b = self.works_v2_bootstrap(r).await?;
        let account = text(&b, "account_id")?;
        let install = text(&b, "installation_id")?;
        let family = text(&b, "family_id")?;
        let now = self.clock.now_ms();
        match operation {
            "identity" => {
                let rows=self.db.query(Statement::new("SELECT a.assistant_id,a.name FROM works_attachments m JOIN works_assistants a ON a.assistant_id=m.assistant_id AND a.account_id=m.account_id WHERE m.account_id=? AND m.installation_id=? AND m.status='active' AND a.status='active' ORDER BY a.name,a.assistant_id",vec![account.into(),install.clone().into()])).await?;
                let assistants = rows
                    .iter()
                    .map(|r| -> Result<Value> {
                        Ok(json!({"assistant_id":text(r,"assistant_id")?,"name":text(r,"name")?}))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(json!({"installation_id":install,"assistants":assistants}))
            }
            "assistant_request" => {
                let name = input
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|v| !v.trim().is_empty() && v.len() <= 160)
                    .ok_or_else(|| Error::invalid("A valid assistant name is required."))?;
                let id = random_id("assistant")?;
                let token = bearer(r)?;
                let (client_guard, client_args) = self.oauth_client_guard("f", now);
                let (identity, identity_args) = self.unified_identity_guard("ac");
                let mut args: Vec<Sql> = vec![
                    id.clone().into(),
                    name.into(),
                    now.into(),
                    account.into(),
                    install.into(),
                    family.into(),
                    Sha256::digest(token.as_bytes()).to_vec().into(),
                    now.into(),
                    now.into(),
                ];
                args.extend(identity_args);
                args.extend(client_args);
                let rows=self.db.batch(vec![Statement::new(format!("INSERT INTO works_assistants(assistant_id,account_id,name,created_at) SELECT ?,f.account_id,?,? FROM works_families f JOIN works_installations i ON i.installation_id=f.installation_id AND i.account_id=f.account_id JOIN works_access_tokens t ON t.family_id=f.family_id JOIN accounts ac ON ac.account_id=f.account_id WHERE f.account_id=? AND i.installation_id=? AND f.family_id=? AND t.token_digest=? AND t.expires_at>? AND f.expires_at>? AND f.revoked_at IS NULL AND i.status='active' AND ac.disabled=0 AND {identity} AND {client_guard} RETURNING assistant_id"),args)]).await?;
                if rows[0].is_empty() {
                    return Err(Error::forbidden());
                }
                Ok(json!({"assistant_id":id,"policy_revision":1}))
            }
            "mount_request" => {
                let assistant = input
                    .get("assistant_id")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| Error::invalid("assistant_id is required."))?;
                let id = random_id("mount")?;
                let token = bearer(r)?;
                let digest = Sha256::digest(token.as_bytes()).to_vec();
                let (client_guard, client_args) = self.oauth_client_guard("f", now);
                let (identity, identity_args) = self.unified_identity_guard("ac");
                let mut args: Vec<Sql> = vec![
                    id.clone().into(),
                    now.into(),
                    (now + 600_000).into(),
                    assistant.into(),
                    account.into(),
                    install.into(),
                    family.into(),
                    digest.into(),
                    now.into(),
                    now.into(),
                ];
                args.extend(identity_args);
                args.extend(client_args);
                let rows=self.db.batch(vec![Statement::new(format!("INSERT INTO works_mount_requests(request_id,account_id,installation_id,family_id,assistant_id,policy_revision,status,created_at,expires_at) SELECT ?,a.account_id,i.installation_id,f.family_id,a.assistant_id,a.policy_revision,'pending',?,? FROM works_assistants a JOIN works_installations i ON i.account_id=a.account_id JOIN works_families f ON f.installation_id=i.installation_id AND f.account_id=i.account_id JOIN works_access_tokens t ON t.family_id=f.family_id JOIN accounts ac ON ac.account_id=f.account_id WHERE a.assistant_id=? AND a.account_id=? AND i.installation_id=? AND f.family_id=? AND t.token_digest=? AND t.expires_at>? AND f.expires_at>? AND f.revoked_at IS NULL AND i.status='active' AND a.status='active' AND ac.disabled=0 AND {identity} AND {client_guard} RETURNING request_id"),args)]).await?;
                if rows[0].is_empty() {
                    return Err(Error::forbidden());
                }
                Ok(
                    json!({"request_id":id,"approval_url":format!("{}/dashboard?mount_request={}",self.config.public_origin,id)}),
                )
            }
            "mount_status" => {
                let id = input
                    .get("request_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Error::invalid("request_id is required."))?;
                let rows=self.db.query(Statement::new("SELECT status,expires_at,replaced_by FROM works_mount_requests WHERE request_id=? AND account_id=? AND installation_id=? AND family_id=?",vec![id.into(),account.into(),install.into(),family.into()])).await?;
                let row = rows.first().ok_or_else(Error::not_found)?;
                let next = optional_text(row, "replaced_by");
                Ok(
                    json!({"status":if next.is_some(){"superseded".to_string()}else{text(row,"status")?},"next_request_id":next,"expires_at":integer(row,"expires_at")?}),
                )
            }
            _ => Err(Error::not_found()),
        }
    }
    pub(super) async fn works_v2_account_handle(&self, r: &Request) -> Response {
        let operation = match (r.method.as_str(), r.path.as_str()) {
            ("POST", "/account/works/sessions/list") => Some("sessions_list"),
            ("POST", "/account/works/sessions/update") => Some("sessions_update"),
            ("POST", "/account/works/recipients/list") => Some("recipients_list"),
            ("POST", "/account/works/messages/read") => Some("messages_read"),
            ("POST", "/account/works/messages/send") => Some("messages_send"),
            ("POST", "/account/works/deliveries/handoff") => Some("deliveries_handoff"),
            _ => None,
        };
        let result = if r.method == "GET" && r.path == "/account/works/bridge/status" {
            self.works_wake_account_status(r).await
        } else if let Some(operation) = operation {
            match crate::json::parse(&r.body, self.limits.max_json_bytes) {
                Ok(v) => self.works_account_dispatch(r, operation, v).await,
                Err(e) => Err(e),
            }
        } else {
            self.works_v2_account_action(r).await
        };
        let mut response = match result {
            Ok(v) => Response::json(200, json!({"ok":true,"data":v})),
            Err(e) => Response::json(e.status(), json!({"ok":false,"error":e.value()})),
        };
        response
            .headers
            .insert("cache-control".into(), "no-store".into());
        response
            .headers
            .insert("referrer-policy".into(), "no-referrer".into());
        response
    }
    async fn works_v2_account_action(&self, r: &Request) -> Result<Value> {
        let session = self.account_session(r, r.method != "GET").await?;
        let account = text(&session, "account_id")?;
        let now = self.clock.now_ms();
        if r.method != "GET" && integer(&session, "created_at")? < now - 900_000 {
            return Err(Error::new(
                "REAUTHENTICATION_REQUIRED",
                "Sign in again before changing permissions.",
            ));
        }
        let digest =
            Sha256::digest(accounts::account_cookie(r, &self.config.public_origin)?.as_bytes())
                .to_vec();
        let active = "EXISTS(SELECT 1 FROM account_sessions s JOIN accounts a ON a.account_id=s.account_id WHERE s.session_digest=? AND s.account_id=? AND s.expires_at>? AND s.created_at>=? AND a.disabled=0)";
        let guard = || -> Vec<Sql> {
            vec![
                digest.clone().into(),
                account.clone().into(),
                now.into(),
                (now - 900_000).into(),
            ]
        };
        match (r.method.as_str(), r.path.as_str()) {
            ("GET", "/account/works/assistants") => {
                let rows=self.db.query(Statement::new("SELECT * FROM works_assistants WHERE account_id=? ORDER BY created_at,assistant_id",vec![account.into()])).await?;
                let out=rows.iter().map(|x|->Result<Value>{Ok(json!({"assistant_id":text(x,"assistant_id")?,"name":text(x,"name")?,"status":text(x,"status")?,"policy_revision":integer(x,"policy_revision")?,"messages_enabled":integer(x,"messages_enabled")?==1,"keys_read":integer(x,"keys_read")?==1,"keys_write":integer(x,"keys_write")?==1,"keys_create":integer(x,"keys_create")?==1,"key_scope":text(x,"key_scope")?,"default_session_id":optional_text(x,"default_session_id")}))}).collect::<Result<Vec<_>>>()?;
                Ok(json!({"assistants":out}))
            }
            ("GET", "/account/works/mount-requests") => {
                let rows=self.db.query(Statement::new("SELECT m.*,i.label FROM works_mount_requests m JOIN works_installations i ON i.installation_id=m.installation_id WHERE m.account_id=? AND m.status='pending' AND m.expires_at>? ORDER BY m.created_at",vec![account.into(),now.into()])).await?;
                let out=rows.iter().map(|x|->Result<Value>{Ok(json!({"request_id":text(x,"request_id")?,"assistant_id":text(x,"assistant_id")?,"installation_id":text(x,"installation_id")?,"policy_revision":integer(x,"policy_revision")?,"expires_at":integer(x,"expires_at")?,"label":text(x,"label")?}))}).collect::<Result<Vec<_>>>()?;
                Ok(json!({"requests":out}))
            }
            ("GET", "/account/works/delegations") => {
                let rows=self.db.query(Statement::new("SELECT attachment_id,sender_assistant_id,actions,revision FROM works_a2a_delegations WHERE account_id=? ORDER BY attachment_id,sender_assistant_id",vec![account.into()])).await?;
                let entries=rows.iter().map(|x| -> Result<Value> {Ok(json!({"attachment_id":text(x,"attachment_id")?,"sender_assistant_id":text(x,"sender_assistant_id")?,"actions":integer(x,"actions")?,"revision":integer(x,"revision")?}))}).collect::<Result<Vec<_>>>()?;
                Ok(json!({"delegations":entries}))
            }
            ("GET", "/account/works/attachments") => {
                let rows=self.db.query(Statement::new("SELECT m.*,i.label FROM works_attachments m JOIN works_installations i ON i.installation_id=m.installation_id WHERE m.account_id=? ORDER BY m.created_at",vec![account.into()])).await?;
                let out=rows.iter().map(|x|->Result<Value>{Ok(json!({"attachment_id":text(x,"attachment_id")?,"assistant_id":text(x,"assistant_id")?,"installation_id":text(x,"installation_id")?,"label":text(x,"label")?,"status":text(x,"status")?,"created_at":integer(x,"created_at")?}))}).collect::<Result<Vec<_>>>()?;
                Ok(json!({"attachments":out}))
            }
            ("POST", path) => {
                let b = crate::json::parse(&r.body, 4096)?;
                let field = |k: &str| -> Result<String> {
                    b.get(k)
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty() && s.len() <= 160)
                        .map(str::to_owned)
                        .ok_or_else(|| Error::invalid("Missing or invalid field."))
                };
                let flag = |k: &str| -> Result<bool> {
                    b.get(k)
                        .and_then(Value::as_bool)
                        .ok_or_else(|| Error::invalid("Missing permission."))
                };
                match path {
                    "/account/works/delegations/set" => {
                        crate::json::fields(
                            &b,
                            &[
                                "attachment_id",
                                "sender_assistant_id",
                                "actions",
                                "revision",
                            ],
                        )?;
                        let attachment = field("attachment_id")?;
                        let sender = field("sender_assistant_id")?;
                        let actions = b
                            .get("actions")
                            .and_then(Value::as_i64)
                            .filter(|n| (0..=31).contains(n))
                            .ok_or_else(|| Error::invalid("Invalid delegation actions."))?;
                        let revision = b
                            .get("revision")
                            .and_then(Value::as_i64)
                            .filter(|n| *n >= 0)
                            .ok_or_else(|| Error::invalid("Invalid delegation revision."))?;
                        let mut args: Vec<Sql> = vec![
                            account.clone().into(),
                            attachment.clone().into(),
                            sender.clone().into(),
                            actions.into(),
                            now.into(),
                            now.into(),
                            account.clone().into(),
                            attachment.clone().into(),
                            sender.clone().into(),
                            actions.into(),
                        ];
                        args.extend(guard());
                        args.extend([
                            revision.into(),
                            account.clone().into(),
                            attachment.clone().into(),
                            sender.clone().into(),
                            revision.into(),
                            revision.into(),
                        ]);
                        let query = format!(
                            "INSERT INTO works_a2a_delegations(account_id,attachment_id,sender_assistant_id,actions,revision,created_at,updated_at) SELECT ?,?,?,?,1,?,? WHERE EXISTS(SELECT 1 FROM works_attachments a JOIN works_assistants source ON source.account_id=a.account_id WHERE a.account_id=? AND a.attachment_id=? AND source.assistant_id=? AND (a.status='active' AND source.status='active' OR ?=0)) AND {active} AND (?=0 OR EXISTS(SELECT 1 FROM works_a2a_delegations existing WHERE existing.account_id=? AND existing.attachment_id=? AND existing.sender_assistant_id=? AND existing.revision=?)) ON CONFLICT(account_id,attachment_id,sender_assistant_id) DO UPDATE SET actions=excluded.actions,revision=works_a2a_delegations.revision+1,updated_at=excluded.updated_at WHERE works_a2a_delegations.revision=? RETURNING revision"
                        );
                        let rows = self.db.batch(vec![Statement::new(query, args)]).await?;
                        let row = rows[0].first().ok_or_else(|| {
                            Error::new(
                                "REVISION_CONFLICT",
                                "Delegation or attachment changed; reload and review.",
                            )
                        })?;
                        Ok(json!({"revision":integer(row,"revision")?}))
                    }
                    "/account/works/assistants/create" => {
                        let id = random_id("assistant")?;
                        let mut args = vec![
                            id.clone().into(),
                            account.clone().into(),
                            field("name")?.into(),
                            now.into(),
                        ];
                        args.extend(guard());
                        let rows=self.db.batch(vec![Statement::new(format!("INSERT INTO works_assistants(assistant_id,account_id,name,created_at) SELECT ?,?,?,? WHERE {active} RETURNING assistant_id"),args)]).await?;
                        if rows[0].is_empty() {
                            return Err(Error::forbidden());
                        }
                        Ok(json!({"assistant_id":id}))
                    }
                    "/account/works/assistants/policy" => {
                        let rev = b
                            .get("policy_revision")
                            .and_then(Value::as_i64)
                            .ok_or_else(|| Error::invalid("Missing revision."))?;
                        if field("key_scope")? != "all" {
                            return Err(Error::invalid("Selected key scope is not supported yet."));
                        }
                        let mut args = vec![
                            flag("messages_enabled")?.into(),
                            flag("keys_read")?.into(),
                            flag("keys_write")?.into(),
                            flag("keys_create")?.into(),
                            field("assistant_id")?.into(),
                            account.clone().into(),
                            rev.into(),
                        ];
                        args.extend(guard());
                        let rows=self.db.batch(vec![Statement::new(format!("UPDATE works_assistants SET messages_enabled=?,keys_read=?,keys_write=?,keys_create=?,key_scope='all',policy_revision=policy_revision+1 WHERE assistant_id=? AND account_id=? AND policy_revision=? AND {active} RETURNING policy_revision"),args)]).await?;
                        let x = rows[0].first().ok_or_else(|| {
                            Error::new("REVISION_CONFLICT", "Policy changed; reload and review.")
                        })?;
                        Ok(json!({"policy_revision":integer(x,"policy_revision")?}))
                    }
                    "/account/works/mount-requests/review" => {
                        let old = field("request_id")?;
                        let id = random_id("mount")?;
                        let mut args: Vec<Sql> = vec![
                            id.clone().into(),
                            now.into(),
                            old.clone().into(),
                            account.clone().into(),
                            now.into(),
                            now.into(),
                        ];
                        args.extend(guard());
                        let (client_guard, client_args) = self.oauth_client_guard("f", now);
                        args.extend(client_args);
                        let rows=self.db.batch(vec![Statement::new(format!("INSERT INTO works_mount_requests(request_id,account_id,installation_id,family_id,assistant_id,policy_revision,status,created_at,expires_at) SELECT ?,m.account_id,m.installation_id,m.family_id,m.assistant_id,a.policy_revision,'pending',?,m.expires_at FROM works_mount_requests m JOIN works_assistants a ON a.assistant_id=m.assistant_id AND a.account_id=m.account_id JOIN works_installations i ON i.installation_id=m.installation_id AND i.account_id=m.account_id JOIN works_families f ON f.family_id=m.family_id AND f.account_id=m.account_id AND f.installation_id=m.installation_id WHERE m.request_id=? AND m.account_id=? AND m.status='pending' AND m.expires_at>? AND f.expires_at>? AND f.revoked_at IS NULL AND i.status='active' AND a.status='active' AND {active} AND {client_guard} RETURNING policy_revision"),args),Statement::new("UPDATE works_mount_requests SET status='denied',replaced_by=? WHERE request_id=? AND status='pending' AND EXISTS(SELECT 1 FROM works_mount_requests WHERE request_id=?)",vec![id.clone().into(),old.into(),id.clone().into()])]).await?;
                        let row = rows[0].first().ok_or_else(|| {
                            Error::new(
                                "REVISION_CONFLICT",
                                "Request expired or client is no longer available.",
                            )
                        })?;
                        Ok(
                            json!({"request_id":id,"policy_revision":integer(row,"policy_revision")?}),
                        )
                    }
                    "/account/works/mount-requests/approve" => {
                        let id = field("request_id")?;
                        let rev = b
                            .get("policy_revision")
                            .and_then(Value::as_i64)
                            .ok_or_else(|| Error::invalid("Missing revision."))?;
                        let aid = random_id("attachment")?;
                        let mut args = vec![
                            aid.clone().into(),
                            now.into(),
                            id.clone().into(),
                            account.clone().into(),
                            rev.into(),
                            now.into(),
                            now.into(),
                        ];
                        args.extend(guard());
                        let (client_guard, client_args) = self.oauth_client_guard("f", now);
                        args.extend(client_args);
                        let q = Statement::new(
                            format!(
                                "INSERT INTO works_attachments(attachment_id,account_id,assistant_id,installation_id,status,created_at) SELECT ?,m.account_id,m.assistant_id,m.installation_id,'active',? FROM works_mount_requests m JOIN works_assistants a ON a.assistant_id=m.assistant_id AND a.account_id=m.account_id JOIN works_installations i ON i.installation_id=m.installation_id AND i.account_id=m.account_id JOIN works_families f ON f.family_id=m.family_id AND f.installation_id=i.installation_id AND f.account_id=m.account_id WHERE m.request_id=? AND m.account_id=? AND m.policy_revision=? AND m.status='pending' AND m.expires_at>? AND a.policy_revision=m.policy_revision AND a.status='active' AND i.status='active' AND f.revoked_at IS NULL AND f.expires_at>? AND {active} AND {client_guard} AND NOT EXISTS(SELECT 1 FROM works_attachments existing WHERE existing.assistant_id=m.assistant_id AND existing.installation_id=m.installation_id AND existing.status='active') RETURNING attachment_id"
                            ),
                            args,
                        );
                        let rows=self.db.batch(vec![q,Statement::new("UPDATE works_mount_requests SET status='approved' WHERE request_id=? AND status='pending' AND EXISTS(SELECT 1 FROM works_attachments WHERE attachment_id=?)",vec![id.into(),aid.clone().into()])]).await?;
                        if rows[0].is_empty() {
                            return Err(Error::new(
                                "REVISION_CONFLICT",
                                "Request expired or policy changed; request approval again.",
                            ));
                        }
                        Ok(json!({"attachment_id":aid}))
                    }
                    "/account/works/attachments/revoke" => {
                        let id = field("attachment_id")?;
                        let mut args = vec![now.into(), id.clone().into(), account.clone().into()];
                        args.extend(guard());
                        let predicate = "EXISTS(SELECT 1 FROM works_attachments m WHERE m.attachment_id=? AND m.account_id=? AND m.status='revoked' AND m.revoked_at=? AND m.assistant_id=works_deliveries.assistant_id AND m.installation_id IN(SELECT installation_id FROM works_sessions WHERE session_id=works_deliveries.assigned_session_id AND account_id=m.account_id AND assistant_id=m.assistant_id))";
                        let mut delivery = vec![
                            now.into(),
                            id.clone().into(),
                            account.clone().into(),
                            now.into(),
                        ];
                        delivery.extend(guard());
                        let mut sessions =
                            vec![id.clone().into(), account.clone().into(), now.into()];
                        sessions.extend(guard());
                        let mut assistant = vec![
                            now.into(),
                            id.clone().into(),
                            account.clone().into(),
                            now.into(),
                        ];
                        assistant.extend(guard());
                        let rows=self.db.batch(vec![Statement::new(format!("UPDATE works_attachments SET status='revoked',revoked_at=? WHERE attachment_id=? AND account_id=? AND status='active' AND {active} RETURNING attachment_id"),args),
                            Statement::new(format!("UPDATE works_deliveries SET state='needs_handoff',assignment_generation=assignment_generation+1,updated_at=? WHERE state IN('assigned','claimed','unknown') AND {predicate} AND {active}"),delivery),
                            Statement::new(format!("UPDATE works_sessions SET route_revision=route_revision+1 WHERE EXISTS(SELECT 1 FROM works_attachments m WHERE m.attachment_id=? AND m.account_id=? AND m.revoked_at=? AND m.status='revoked' AND m.account_id=works_sessions.account_id AND m.assistant_id=works_sessions.assistant_id AND m.installation_id=works_sessions.installation_id) AND {active}"),sessions),
                            Statement::new(format!("UPDATE works_assistants SET default_session_id=(SELECT s.session_id FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=works_assistants.account_id AND s.assistant_id=works_assistants.assistant_id AND s.status='registered' AND i.status='active' AND t.status='active' ORDER BY CASE WHEN s.capability='wakeable' AND s.last_seen_at>=?-120000 THEN 0 ELSE 1 END,s.created_at,s.session_id LIMIT 1),route_revision=route_revision+1 WHERE EXISTS(SELECT 1 FROM works_attachments m JOIN works_sessions old ON old.account_id=m.account_id AND old.assistant_id=m.assistant_id AND old.installation_id=m.installation_id WHERE m.attachment_id=? AND m.account_id=? AND m.revoked_at=? AND m.status='revoked' AND m.account_id=works_assistants.account_id AND m.assistant_id=works_assistants.assistant_id AND old.session_id=works_assistants.default_session_id) AND {active}"),assistant)]).await?;
                        if rows[0].is_empty() {
                            return Err(Error::not_found());
                        }
                        Ok(json!({}))
                    }
                    _ => Err(Error::not_found()),
                }
            }
            _ => Err(Error::not_found()),
        }
    }
}
