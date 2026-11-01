//! OAuth token endpoint state transitions. Secrets are returned only once.
use super::oauth::oauth_error;
use super::*;

const ACCESS_MS: i64 = 300_000;
const FAMILY_MS: i64 = 30 * 24 * 60 * 60 * 1000;
fn digest(value: &str) -> Sql {
    Sha256::digest(value.as_bytes()).to_vec().into()
}
fn required<'a>(p: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str> {
    p.get(key)
        .filter(|s| !s.is_empty())
        .map(String::as_str)
        .ok_or_else(|| oauth_error("invalid_request"))
}
fn fields(p: &BTreeMap<String, String>, allowed: &[&str]) -> Result<()> {
    if p.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(oauth_error("invalid_request"));
    }
    Ok(())
}
fn opaque(value: &str) -> Result<()> {
    if !(20..=256).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(oauth_error("invalid_grant"));
    }
    Ok(())
}
fn verifier(value: &str) -> Result<String> {
    if !(43..=128).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
    {
        return Err(oauth_error("invalid_grant"));
    }
    Ok(URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes())))
}
fn secret(prefix: &str) -> Result<String> {
    Ok(format!(
        "{prefix}{}",
        URL_SAFE_NO_PAD.encode(random_bytes::<32>()?)
    ))
}

// Alias f is always an OAuth family. This is rechecked in the issuing batch.
const LIVE_FAMILY: &str = "f.revoked_at IS NULL AND f.expires_at>? AND EXISTS(SELECT 1 FROM accounts a JOIN account_spaces m ON m.account_id=a.account_id JOIN actors x ON x.space_id=m.space_id JOIN consumers c ON c.space_id=x.space_id AND c.actor_id=x.actor_id WHERE a.account_id=f.account_id AND m.space_id=f.space_id AND x.actor_id=f.actor_id AND c.consumer_id=f.consumer_id AND a.disabled=0 AND x.disabled=0 AND c.disabled=0 AND c.view='inbox')";
// Alias q is the original approved authorization request.
const LIVE_REQUEST: &str = "EXISTS(SELECT 1 FROM accounts a JOIN account_spaces m ON m.account_id=a.account_id JOIN actors x ON x.space_id=m.space_id JOIN consumers c ON c.space_id=x.space_id AND c.actor_id=x.actor_id WHERE a.account_id=q.account_id AND m.space_id=q.space_id AND x.actor_id=q.actor_id AND c.consumer_id=q.consumer_id AND a.disabled=0 AND x.disabled=0 AND c.disabled=0 AND c.view='inbox')";

fn revoke_queries(selector: &str, args: Vec<Sql>, now: i64) -> Vec<Statement> {
    let mut first = vec![now.into()];
    first.extend(args.clone());
    let mut second = vec![now.into()];
    second.extend(args);
    vec![
        Statement::new(
            format!(
                "UPDATE oauth_families SET revoked_at=COALESCE(revoked_at,?) WHERE family_id IN ({selector})"
            ),
            first,
        ),
        Statement::new(
            format!(
                "UPDATE tokens SET revoked_at=COALESCE(revoked_at,?),authority_version=authority_version+CASE WHEN revoked_at IS NULL THEN 1 ELSE 0 END WHERE token_id IN(SELECT token_id FROM oauth_access_tokens WHERE family_id IN ({selector}))"
            ),
            second,
        ),
    ]
}

// A family ending must not leave its installation as an active receiver. The
// selector identifies only the family being revoked and is evaluated inside
// the same D1 batch as revocation; another live family keeps the installation.
fn works_revoked_routes(selector: &str, selector_args: Vec<Sql>, now: i64) -> Vec<Statement> {
    let target = format!(
        "EXISTS(SELECT 1 FROM works_families f WHERE f.family_id IN ({selector}) AND f.revoked_at IS NOT NULL AND f.account_id=works_attachments.account_id AND f.installation_id=works_attachments.installation_id) AND NOT EXISTS(SELECT 1 FROM works_families live WHERE live.account_id=works_attachments.account_id AND live.installation_id=works_attachments.installation_id AND live.revoked_at IS NULL AND live.expires_at>?)"
    );
    let mut attachments = vec![now.into()];
    attachments.extend(selector_args.clone());
    attachments.push(now.into());
    let affected = format!(
        "EXISTS(SELECT 1 FROM works_attachments m JOIN works_families f ON f.account_id=m.account_id AND f.installation_id=m.installation_id WHERE f.family_id IN ({selector}) AND f.revoked_at IS NOT NULL AND m.status='revoked' AND m.revoked_at=? AND m.account_id=works_deliveries.account_id AND m.assistant_id=works_deliveries.assistant_id AND EXISTS(SELECT 1 FROM works_sessions s WHERE s.account_id=m.account_id AND s.assistant_id=m.assistant_id AND s.installation_id=m.installation_id AND s.session_id=works_deliveries.assigned_session_id))"
    );
    let mut deliveries = vec![now.into()];
    deliveries.extend(selector_args.clone());
    deliveries.push(now.into());
    let mut sessions = selector_args.clone();
    sessions.push(now.into());
    let mut defaults = vec![now.into()];
    defaults.extend(selector_args);
    defaults.push(now.into());
    vec![
        Statement::new(
            format!(
                "UPDATE works_attachments SET status='revoked',revoked_at=? WHERE status='active' AND {target}"
            ),
            attachments,
        ),
        Statement::new(
            format!(
                "UPDATE works_deliveries SET state='needs_handoff',assignment_generation=assignment_generation+1,updated_at=? WHERE state IN('assigned','claimed','unknown') AND {affected}"
            ),
            deliveries,
        ),
        Statement::new(
            format!(
                "UPDATE works_sessions SET route_revision=route_revision+1 WHERE EXISTS(SELECT 1 FROM works_attachments m JOIN works_families f ON f.account_id=m.account_id AND f.installation_id=m.installation_id WHERE f.family_id IN ({selector}) AND f.revoked_at IS NOT NULL AND m.status='revoked' AND m.revoked_at=? AND m.account_id=works_sessions.account_id AND m.assistant_id=works_sessions.assistant_id AND m.installation_id=works_sessions.installation_id)"
            ),
            sessions,
        ),
        Statement::new(
            format!(
                "UPDATE works_assistants SET default_session_id=(SELECT s.session_id FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=works_assistants.account_id AND s.assistant_id=works_assistants.assistant_id AND s.status='registered' AND i.status='active' AND t.status='active' ORDER BY CASE WHEN s.capability='wakeable' AND s.last_seen_at>=?-120000 THEN 0 ELSE 1 END,s.created_at,s.session_id LIMIT 1),route_revision=route_revision+1 WHERE EXISTS(SELECT 1 FROM works_attachments m JOIN works_families f ON f.account_id=m.account_id AND f.installation_id=m.installation_id JOIN works_sessions old ON old.account_id=m.account_id AND old.assistant_id=m.assistant_id AND old.installation_id=m.installation_id WHERE f.family_id IN ({selector}) AND f.revoked_at IS NOT NULL AND m.status='revoked' AND m.revoked_at=? AND m.account_id=works_assistants.account_id AND m.assistant_id=works_assistants.assistant_id AND old.session_id=works_assistants.default_session_id)"
            ),
            defaults,
        ),
    ]
}

impl<D: Database, O: ObjectStore> App<D, O> {
    pub(super) async fn oauth_token(&self, p: BTreeMap<String, String>) -> Result<Value> {
        let client = required(&p, "client_id")?;
        let resolved_client = self.oauth_client(client).await?;
        if resolved_client.source != "legacy"
            && p.get("grant_type")
                .is_some_and(|v| v == "urn:ietf:params:oauth:grant-type:device_code")
        {
            return Err(oauth_error("unauthorized_client"));
        }
        let resource = required(&p, "resource")?;
        if resource != self.oauth_resource() {
            return Err(oauth_error("invalid_target"));
        }
        if self.keys.is_some() && self.keys_db.is_none() {
            return self.keys_oauth_token(p).await;
        }
        if self.keys_db.is_none() && p.get("scope").is_some_and(|s| s != "waypost") {
            return Err(oauth_error("invalid_scope"));
        }
        if self.keys_db.is_some() {
            let works = match p.get("grant_type").map(String::as_str) {
                Some("authorization_code") => {
                    let rows=self.db.query(Statement::new("SELECT 1 AS ok FROM oauth_requests WHERE code_digest=? AND client_id=? AND resource=? AND scope='works:client'",vec![digest(required(&p,"code")?),client.into(),resource.into()])).await?;
                    !rows.is_empty()
                }
                Some("refresh_token") => {
                    let rows=self.db.query(Statement::new("SELECT 1 AS ok FROM works_refresh_tokens r JOIN works_families f ON f.family_id=r.family_id WHERE r.token_digest=? AND f.client_id=? AND f.resource=?",vec![digest(required(&p,"refresh_token")?),client.into(),resource.into()])).await?;
                    !rows.is_empty()
                }
                _ => false,
            };
            if works {
                return self.works_oauth_token(&p).await;
            }
        }
        let requested_scope = p
            .get("scope")
            .map(|s| self.normalize_scope(Some(s)))
            .transpose()?;
        match required(&p, "grant_type")? {
            "authorization_code" => {
                fields(
                    &p,
                    &[
                        "grant_type",
                        "client_id",
                        "resource",
                        "scope",
                        "code",
                        "redirect_uri",
                        "code_verifier",
                    ],
                )?;
                let code = required(&p, "code")?;
                opaque(code)?;
                let challenge = verifier(required(&p, "code_verifier")?)?;
                let redirect = required(&p, "redirect_uri")?;
                if !self.oauth_client(client).await?.accepts_redirect(redirect) {
                    return Err(oauth_error("invalid_grant"));
                }
                let now = self.clock.now_ms();
                let nonce = random_id("redeem")?;
                let selector = "SELECT family_id FROM oauth_requests WHERE kind='code' AND code_digest=? AND client_id=? AND resource=? AND redirect_uri=? AND code_challenge=? AND status='consumed'";
                let tuple = vec![
                    digest(code),
                    client.into(),
                    resource.into(),
                    redirect.into(),
                    challenge.clone().into(),
                ];
                let mut queries = revoke_queries(selector, tuple.clone(), now);
                let mut claim = vec![nonce.clone().into()];
                claim.extend(tuple);
                claim.push(now.into());
                let (client_guard, client_args) = self.oauth_client_guard("q", now);
                let live_request = if self.keys_db.is_some() {
                    let (identity_guard, identity_args) = self.unified_identity_guard("a");
                    claim.extend(identity_args);
                    LIVE_REQUEST.replace(
                        "a.disabled=0",
                        &format!("a.disabled=0 AND {identity_guard}"),
                    )
                } else {
                    LIVE_REQUEST.to_owned()
                };
                claim.extend(client_args);
                claim.extend([
                    requested_scope.clone().into(),
                    requested_scope.clone().into(),
                ]);

                queries.push(Statement::new(format!("UPDATE oauth_requests AS q SET status='consumed',claim_nonce=? WHERE kind='code' AND code_digest=? AND client_id=? AND resource=? AND redirect_uri=? AND code_challenge=? AND status='approved' AND expires_at>? AND {live_request} AND {client_guard} AND (? IS NULL OR q.scope=?)"),claim));
                self.issue_request(queries, &nonce, now).await
            }
            "refresh_token" => {
                fields(
                    &p,
                    &[
                        "grant_type",
                        "client_id",
                        "resource",
                        "scope",
                        "refresh_token",
                    ],
                )?;
                self.rotate_refresh(
                    client,
                    resource,
                    required(&p, "refresh_token")?,
                    requested_scope.as_deref(),
                )
                .await
            }
            "urn:ietf:params:oauth:grant-type:device_code" => {
                fields(
                    &p,
                    &[
                        "grant_type",
                        "client_id",
                        "resource",
                        "scope",
                        "device_code",
                    ],
                )?;
                self.redeem_device(client, resource, required(&p, "device_code")?)
                    .await
            }
            _ => Err(oauth_error("unsupported_grant_type")),
        }
    }
    async fn keys_oauth_token(&self, p: BTreeMap<String, String>) -> Result<Value> {
        let client = required(&p, "client_id")?;
        let resource = required(&p, "resource")?;
        let requested_scope = p
            .get("scope")
            .map(|scope| self.normalize_scope(Some(scope)))
            .transpose()?;
        let now = self.clock.now_ms();
        let issuer = self
            .oidc
            .as_ref()
            .ok_or_else(|| oauth_error("invalid_grant"))?
            .issuer
            .clone();
        let owner = self
            .account_owner_subject
            .clone()
            .ok_or_else(|| oauth_error("invalid_grant"))?;
        match required(&p, "grant_type")? {
            "authorization_code" => {
                fields(
                    &p,
                    &[
                        "grant_type",
                        "client_id",
                        "resource",
                        "scope",
                        "code",
                        "redirect_uri",
                        "code_verifier",
                    ],
                )?;
                let code = required(&p, "code")?;
                opaque(code)?;
                let challenge = verifier(required(&p, "code_verifier")?)?;
                let redirect = required(&p, "redirect_uri")?;
                if !self.oauth_client(client).await?.accepts_redirect(redirect) {
                    return Err(oauth_error("invalid_grant"));
                }
                let now = self.clock.now_ms();
                let nonce = random_id("redeem")?;
                let family = random_id("grant")?;
                let access = secret("kso_")?;
                let refresh = secret("ksr_")?;
                let (guard, guard_args) = self.oauth_client_guard("q", now);
                let mut claim = vec![
                    nonce.clone().into(),
                    digest(code),
                    client.into(),
                    resource.into(),
                    redirect.into(),
                    challenge.clone().into(),
                    now.into(),
                    issuer.clone().into(),
                    owner.clone().into(),
                    requested_scope.clone().into(),
                    requested_scope.clone().into(),
                ];
                claim.extend(guard_args);
                let rows = self.db.batch(vec![
                    Statement::new("UPDATE key_oauth_families SET revoked_at=COALESCE(revoked_at,?) WHERE family_id IN(SELECT family_id FROM oauth_requests WHERE kind='code' AND code_digest=? AND client_id=? AND resource=? AND redirect_uri=? AND code_challenge=? AND status='consumed')",vec![now.into(),digest(code),client.into(),resource.into(),redirect.into(),challenge.into()]),
                    Statement::new(format!("UPDATE oauth_requests AS q SET status='consumed',claim_nonce=? WHERE kind='code' AND code_digest=? AND client_id=? AND resource=? AND redirect_uri=? AND code_challenge=? AND status='approved' AND expires_at>? AND EXISTS(SELECT 1 FROM accounts a JOIN key_consents k ON k.account_id=a.account_id WHERE k.request_id=q.request_id AND a.account_id=q.account_id AND a.disabled=0 AND a.issuer=? AND a.subject=?) AND (? IS NULL OR q.scope=?) AND {guard}"),claim),
                    Statement::new("INSERT INTO key_oauth_families(family_id,request_id,client_id,client_revision,resource,scope,account_id,created_at,expires_at) SELECT ?,request_id,client_id,client_revision,resource,scope,account_id,?,? FROM oauth_requests WHERE claim_nonce=? AND status='consumed'",vec![family.clone().into(),now.into(),(now+FAMILY_MS).into(),nonce.clone().into()]),
                    Statement::new("UPDATE oauth_requests SET family_id=? WHERE claim_nonce=? AND status='consumed' AND EXISTS(SELECT 1 FROM key_oauth_families WHERE family_id=?)",vec![family.clone().into(),nonce.into(),family.clone().into()]),
                    Statement::new("INSERT INTO key_access_tokens(token_digest,family_id,scope,expires_at,created_at) SELECT ?,family_id,scope,?,? FROM key_oauth_families WHERE family_id=? RETURNING scope",vec![digest(&access),(now+ACCESS_MS).into(),now.into(),family.clone().into()]),
                    Statement::new("INSERT INTO key_oauth_refresh_tokens(token_digest,family_id,expires_at) SELECT ?,family_id,expires_at FROM key_oauth_families WHERE family_id=?",vec![digest(&refresh),family.into()]),
                ]).await?;
                let row = rows[4]
                    .first()
                    .ok_or_else(|| oauth_error("invalid_grant"))?;
                Ok(
                    json!({"access_token":access,"token_type":"Bearer","expires_in":ACCESS_MS/1000,"refresh_token":refresh,"scope":text(row,"scope")?}),
                )
            }
            "refresh_token" => {
                fields(
                    &p,
                    &[
                        "grant_type",
                        "client_id",
                        "resource",
                        "scope",
                        "refresh_token",
                    ],
                )?;
                let raw = required(&p, "refresh_token")?;
                opaque(raw)?;
                let nonce = random_id("refresh")?;
                let access = secret("kso_")?;
                let refresh = secret("ksr_")?;
                let (guard, guard_args) = self.oauth_client_guard("f", now);
                let mut args = vec![
                    now.into(),
                    nonce.clone().into(),
                    digest(raw),
                    now.into(),
                    client.into(),
                    resource.into(),
                    now.into(),
                    issuer.into(),
                    owner.into(),
                    requested_scope.clone().into(),
                    requested_scope.into(),
                ];
                args.extend(guard_args);
                let rows = self.db.batch(vec![
                    Statement::new("UPDATE key_oauth_families SET revoked_at=COALESCE(revoked_at,?) WHERE family_id IN(SELECT r.family_id FROM key_oauth_refresh_tokens r JOIN key_oauth_families f ON f.family_id=r.family_id WHERE r.token_digest=? AND f.client_id=? AND f.resource=? AND r.used_at IS NOT NULL)",vec![now.into(),digest(raw),client.into(),resource.into()]),
                    Statement::new(format!("UPDATE key_oauth_refresh_tokens SET used_at=?,redeem_nonce=? WHERE token_digest=? AND used_at IS NULL AND expires_at>? AND EXISTS(SELECT 1 FROM key_oauth_families f JOIN accounts a ON a.account_id=f.account_id WHERE f.family_id=key_oauth_refresh_tokens.family_id AND f.client_id=? AND f.resource=? AND f.revoked_at IS NULL AND f.expires_at>? AND a.disabled=0 AND a.issuer=? AND a.subject=? AND (? IS NULL OR f.scope=?) AND {guard})"),args),
                    Statement::new("INSERT INTO key_access_tokens(token_digest,family_id,scope,expires_at,created_at) SELECT ?,f.family_id,f.scope,MIN(?,f.expires_at),? FROM key_oauth_refresh_tokens r JOIN key_oauth_families f ON f.family_id=r.family_id WHERE r.redeem_nonce=? RETURNING expires_at,scope",vec![digest(&access),(now+ACCESS_MS).into(),now.into(),nonce.clone().into()]),
                    Statement::new("INSERT INTO key_oauth_refresh_tokens(token_digest,family_id,expires_at) SELECT ?,family_id,expires_at FROM key_oauth_refresh_tokens WHERE redeem_nonce=?",vec![digest(&refresh),nonce.into()]),
                ]).await?;
                let row = rows[2]
                    .first()
                    .ok_or_else(|| oauth_error("invalid_grant"))?;
                Ok(
                    json!({"access_token":access,"token_type":"Bearer","expires_in":(integer(row,"expires_at")?-now)/1000,"refresh_token":refresh,"scope":text(row,"scope")?}),
                )
            }
            _ => Err(oauth_error("unsupported_grant_type")),
        }
    }
    async fn works_oauth_token(&self, p: &BTreeMap<String, String>) -> Result<Value> {
        let client = required(p, "client_id")?;
        let resource = required(p, "resource")?;
        let now = self.clock.now_ms();
        if p.get("scope").is_some_and(|v| v != "works:client") {
            return Err(oauth_error("invalid_scope"));
        }
        let nonce = random_id("redeem")?;
        let access = secret("wvo_")?;
        let refresh = secret("wvr_")?;
        match required(p, "grant_type")? {
            "authorization_code" => {
                fields(
                    p,
                    &[
                        "grant_type",
                        "client_id",
                        "resource",
                        "scope",
                        "code",
                        "redirect_uri",
                        "code_verifier",
                    ],
                )?;
                let code = required(p, "code")?;
                opaque(code)?;
                let redirect = required(p, "redirect_uri")?;
                if !self.oauth_client(client).await?.accepts_redirect(redirect) {
                    return Err(oauth_error("invalid_grant"));
                }
                let challenge = verifier(required(p, "code_verifier")?)?;
                let family = random_id("family")?;
                let installation = random_id("installation")?;
                let (client_guard, client_args) = self.oauth_client_guard("q", now);
                let (identity, identity_args) = self.unified_identity_guard("a");
                let mut claim: Vec<Sql> = vec![
                    nonce.clone().into(),
                    digest(code),
                    client.into(),
                    resource.into(),
                    redirect.into(),
                    challenge.clone().into(),
                    now.into(),
                ];
                claim.extend(identity_args);
                claim.extend(client_args);
                let replay_selector = "SELECT family_id FROM oauth_requests WHERE code_digest=? AND client_id=? AND resource=? AND redirect_uri=? AND code_challenge=? AND status='consumed'";
                let replay_args = vec![
                    digest(code),
                    client.into(),
                    resource.into(),
                    redirect.into(),
                    challenge.clone().into(),
                ];
                let mut q = vec![
                    Statement::new(
                        "UPDATE works_families SET revoked_at=COALESCE(revoked_at,?) WHERE family_id IN (SELECT family_id FROM oauth_requests WHERE code_digest=? AND client_id=? AND resource=? AND redirect_uri=? AND code_challenge=? AND status='consumed')",
                        vec![
                            now.into(),
                            digest(code),
                            client.into(),
                            resource.into(),
                            redirect.into(),
                            challenge.into(),
                        ],
                    ),
                    Statement::new(
                        format!(
                            "UPDATE oauth_requests AS q SET status='consumed',claim_nonce=? WHERE code_digest=? AND client_id=? AND resource=? AND redirect_uri=? AND code_challenge=? AND status='approved' AND scope='works:client' AND expires_at>? AND EXISTS(SELECT 1 FROM accounts a WHERE a.account_id=q.account_id AND a.disabled=0 AND {identity}) AND {client_guard} AND (NOT EXISTS(SELECT 1 FROM works_oauth_mount_intents mi WHERE mi.request_id=q.request_id) OR EXISTS(SELECT 1 FROM works_oauth_mount_intents mi JOIN works_assistants a ON a.assistant_id=mi.assistant_id AND a.account_id=mi.account_id WHERE mi.request_id=q.request_id AND mi.account_id=q.account_id AND a.status='active' AND a.policy_revision=mi.policy_revision))"
                        ),
                        claim,
                    ),
                    Statement::new(
                        "INSERT INTO works_installations(installation_id,account_id,label,created_at) SELECT ?,account_id,'Client',? FROM oauth_requests WHERE claim_nonce=? AND status='consumed'",
                        vec![
                            installation.clone().into(),
                            now.into(),
                            nonce.clone().into(),
                        ],
                    ),
                    Statement::new(
                        "INSERT INTO works_families(family_id,account_id,client_id,client_revision,resource,installation_id,created_at,expires_at) SELECT ?,account_id,client_id,client_revision,resource,?,?,? FROM oauth_requests WHERE claim_nonce=? AND status='consumed'",
                        vec![
                            family.clone().into(),
                            installation.clone().into(),
                            now.into(),
                            (now + FAMILY_MS).into(),
                            nonce.clone().into(),
                        ],
                    ),
                    Statement::new(
                        "UPDATE oauth_requests SET family_id=? WHERE claim_nonce=? AND status='consumed'",
                        vec![family.clone().into(), nonce.clone().into()],
                    ),
                    Statement::new(
                        "INSERT INTO works_access_tokens(token_digest,family_id,expires_at) SELECT ?,family_id,? FROM works_families WHERE family_id=? RETURNING expires_at",
                        vec![
                            digest(&access),
                            (now + ACCESS_MS).into(),
                            family.clone().into(),
                        ],
                    ),
                    Statement::new(
                        "INSERT INTO works_refresh_tokens(token_digest,family_id,expires_at) SELECT ?,family_id,expires_at FROM works_families WHERE family_id=?",
                        vec![digest(&refresh), family.into()],
                    ),
                ];
                q.push(Statement::new(
                    "INSERT INTO works_attachments(attachment_id,account_id,assistant_id,installation_id,status,created_at) SELECT ?,mi.account_id,mi.assistant_id,i.installation_id,'active',? FROM oauth_requests q JOIN works_oauth_mount_intents mi ON mi.request_id=q.request_id AND mi.account_id=q.account_id JOIN works_installations i ON i.installation_id=? AND i.account_id=q.account_id JOIN works_assistants a ON a.assistant_id=mi.assistant_id AND a.account_id=mi.account_id WHERE q.claim_nonce=? AND q.status='consumed' AND a.status='active' AND a.policy_revision=mi.policy_revision",
                    vec![random_id("attachment")?.into(),now.into(),installation.into(),nonce.clone().into()],
                ));
                q.extend(works_revoked_routes(replay_selector, replay_args, now));
                let rows = self.db.batch(q).await?;
                if rows[5].is_empty() {
                    return Err(oauth_error("invalid_grant"));
                }
                Ok(
                    json!({"access_token":access,"token_type":"Bearer","expires_in":ACCESS_MS/1000,"refresh_token":refresh,"scope":"works:client"}),
                )
            }
            "refresh_token" => {
                fields(
                    p,
                    &[
                        "grant_type",
                        "client_id",
                        "resource",
                        "scope",
                        "refresh_token",
                    ],
                )?;
                let raw = required(p, "refresh_token")?;
                opaque(raw)?;
                let (client_guard, client_args) = self.oauth_client_guard("f", now);
                let (identity, identity_args) = self.unified_identity_guard("a");
                let mut args: Vec<Sql> = vec![
                    now.into(),
                    nonce.clone().into(),
                    digest(raw),
                    now.into(),
                    client.into(),
                    resource.into(),
                    now.into(),
                ];
                args.extend(identity_args);
                args.extend(client_args);
                let replay_selector = "SELECT r.family_id FROM works_refresh_tokens r JOIN works_families f ON f.family_id=r.family_id WHERE r.token_digest=? AND r.used_at IS NOT NULL AND f.client_id=? AND f.resource=?";
                let replay_args = vec![digest(raw), client.into(), resource.into()];
                let mut queries = vec![
                    Statement::new(
                        "UPDATE works_families SET revoked_at=COALESCE(revoked_at,?) WHERE family_id IN (SELECT r.family_id FROM works_refresh_tokens r JOIN works_families f ON f.family_id=r.family_id WHERE r.token_digest=? AND r.used_at IS NOT NULL AND f.client_id=? AND f.resource=?)",
                        vec![now.into(), digest(raw), client.into(), resource.into()],
                    ),
                    Statement::new(
                        format!(
                            "UPDATE works_refresh_tokens SET used_at=?,redeem_nonce=? WHERE token_digest=? AND used_at IS NULL AND expires_at>? AND EXISTS(SELECT 1 FROM works_families f JOIN accounts a ON a.account_id=f.account_id JOIN works_installations i ON i.installation_id=f.installation_id AND i.account_id=f.account_id WHERE f.family_id=works_refresh_tokens.family_id AND f.client_id=? AND f.resource=? AND f.revoked_at IS NULL AND f.expires_at>? AND a.disabled=0 AND i.status='active' AND {identity} AND {client_guard})"
                        ),
                        args,
                    ),
                    Statement::new(
                        "INSERT INTO works_access_tokens(token_digest,family_id,expires_at) SELECT ?,f.family_id,MIN(?,f.expires_at) FROM works_refresh_tokens r JOIN works_families f ON f.family_id=r.family_id WHERE r.redeem_nonce=? AND f.revoked_at IS NULL RETURNING expires_at",
                        vec![
                            digest(&access),
                            (now + ACCESS_MS).into(),
                            nonce.clone().into(),
                        ],
                    ),
                    Statement::new(
                        "INSERT INTO works_refresh_tokens(token_digest,family_id,expires_at) SELECT ?,r.family_id,r.expires_at FROM works_refresh_tokens r JOIN works_families f ON f.family_id=r.family_id WHERE r.redeem_nonce=? AND f.revoked_at IS NULL",
                        vec![digest(&refresh), nonce.into()],
                    ),
                ];
                queries.extend(works_revoked_routes(replay_selector, replay_args, now));
                let rows = self.db.batch(queries).await?;
                let row = rows[2]
                    .first()
                    .ok_or_else(|| oauth_error("invalid_grant"))?;
                Ok(
                    json!({"access_token":access,"token_type":"Bearer","expires_in":(integer(row,"expires_at")?-now)/1000,"refresh_token":refresh,"scope":"works:client"}),
                )
            }
            _ => Err(oauth_error("unsupported_grant_type")),
        }
    }
    async fn issue_request(
        &self,
        mut queries: Vec<Statement>,
        nonce: &str,
        now: i64,
    ) -> Result<Value> {
        let family = random_id("grant")?;
        let token_id = random_id("token")?;
        let access = secret("wpo_")?;
        let refresh = secret("wpr_")?;
        queries.push(Statement::new("INSERT INTO oauth_families(family_id,client_id,resource,scope,account_id,space_id,actor_id,consumer_id,client_revision,created_at,expires_at) SELECT ?,client_id,resource,scope,account_id,space_id,actor_id,consumer_id,client_revision,?,? FROM oauth_requests WHERE claim_nonce=? AND status='consumed'",vec![family.clone().into(),now.into(),(now+FAMILY_MS).into(),nonce.into()]));
        queries.push(Statement::new("UPDATE oauth_requests SET family_id=? WHERE claim_nonce=? AND status='consumed' AND EXISTS(SELECT 1 FROM oauth_families WHERE family_id=?)",vec![family.clone().into(),nonce.into(),family.clone().into()]));
        queries.push(Statement::new("INSERT INTO tokens(token_id,token_digest,space_id,actor_id,consumer_id,expires_at,created_at,credential_kind) SELECT ?,?,space_id,actor_id,consumer_id,?,?,'oauth' FROM oauth_families WHERE family_id=? RETURNING token_id,(SELECT scope FROM oauth_families WHERE family_id=?) AS scope",vec![token_id.clone().into(),digest(&access),(now+ACCESS_MS).into(),now.into(),family.clone().into(),family.clone().into()]));
        let result_index = queries.len() - 1;
        queries.push(Statement::new("INSERT INTO oauth_access_tokens(token_id,family_id) SELECT token_id,? FROM tokens WHERE token_id=?",vec![family.clone().into(),token_id.into()]));
        queries.push(Statement::new("INSERT INTO oauth_refresh_tokens(token_digest,family_id,expires_at) SELECT ?,family_id,expires_at FROM oauth_families WHERE family_id=?",vec![digest(&refresh),family.into()]));
        let results = self.db.batch(queries).await?;
        if results[result_index].is_empty() {
            return Err(oauth_error("invalid_grant"));
        }
        Ok(
            json!({"access_token":access,"token_type":"Bearer","expires_in":ACCESS_MS/1000,"refresh_token":refresh,"scope":text(&results[result_index][0],"scope")?}),
        )
    }
    async fn rotate_refresh(
        &self,
        client: &str,
        resource: &str,
        raw: &str,
        requested_scope: Option<&str>,
    ) -> Result<Value> {
        opaque(raw)?;
        let now = self.clock.now_ms();
        let nonce = random_id("refresh")?;
        let selector = "SELECT r.family_id FROM oauth_refresh_tokens r JOIN oauth_families f ON f.family_id=r.family_id WHERE r.token_digest=? AND f.client_id=? AND f.resource=? AND r.used_at IS NOT NULL";
        let mut q = revoke_queries(
            selector,
            vec![digest(raw), client.into(), resource.into()],
            now,
        );
        let (client_guard, client_args) = self.oauth_client_guard("f", now);
        let mut refresh_args = vec![
            now.into(),
            nonce.clone().into(),
            digest(raw),
            now.into(),
            client.into(),
            resource.into(),
            now.into(),
        ];
        let live_family = if self.keys_db.is_some() {
            let (identity_guard, identity_args) = self.unified_identity_guard("a");
            refresh_args.extend(identity_args);
            format!(
                "f.revoked_at IS NULL AND f.expires_at>? AND EXISTS(SELECT 1 FROM accounts a WHERE a.account_id=f.account_id AND a.disabled=0 AND {identity_guard}) AND EXISTS(SELECT 1 FROM oauth_requests uq JOIN unified_connection_grants ug ON ug.request_id=uq.request_id AND ug.account_id=f.account_id WHERE uq.family_id=f.family_id)"
            )
        } else {
            LIVE_FAMILY.to_owned()
        };
        refresh_args.extend(client_args);
        refresh_args.extend([
            requested_scope.map(str::to_owned).into(),
            requested_scope.map(str::to_owned).into(),
        ]);
        q.push(Statement::new(format!("UPDATE oauth_refresh_tokens SET used_at=?,redeem_nonce=? WHERE token_digest=? AND used_at IS NULL AND expires_at>? AND EXISTS(SELECT 1 FROM oauth_families f WHERE f.family_id=oauth_refresh_tokens.family_id AND f.client_id=? AND f.resource=? AND {live_family} AND {client_guard} AND (? IS NULL OR f.scope=?))"),refresh_args));
        let token_id = random_id("token")?;
        let access = secret("wpo_")?;
        let refresh = secret("wpr_")?;
        // The old digest remains for replay detection. The family has an absolute expiry.
        q.push(Statement::new("INSERT INTO tokens(token_id,token_digest,space_id,actor_id,consumer_id,expires_at,created_at,credential_kind) SELECT ?,?,f.space_id,f.actor_id,f.consumer_id,MIN(?,f.expires_at),?,'oauth' FROM oauth_refresh_tokens r JOIN oauth_families f ON f.family_id=r.family_id WHERE r.redeem_nonce=? RETURNING expires_at,(SELECT f.scope FROM oauth_refresh_tokens r JOIN oauth_families f ON f.family_id=r.family_id WHERE r.redeem_nonce=?) AS scope",vec![token_id.clone().into(),digest(&access),(now+ACCESS_MS).into(),now.into(),nonce.clone().into(),nonce.clone().into()]));
        q.push(Statement::new("INSERT INTO oauth_access_tokens(token_id,family_id) SELECT ?,family_id FROM oauth_refresh_tokens WHERE redeem_nonce=?",vec![token_id.into(),nonce.clone().into()]));
        q.push(Statement::new("INSERT INTO oauth_refresh_tokens(token_digest,family_id,expires_at) SELECT ?,family_id,expires_at FROM oauth_refresh_tokens WHERE redeem_nonce=?",vec![digest(&refresh),nonce.into()]));
        let rows = self.db.batch(q).await?;
        let row = rows[3]
            .first()
            .ok_or_else(|| oauth_error("invalid_grant"))?;
        Ok(
            json!({"access_token":access,"token_type":"Bearer","expires_in":(integer(row,"expires_at")?-now)/1000,"refresh_token":refresh,"scope":text(row,"scope")?}),
        )
    }
    async fn redeem_device(&self, client: &str, resource: &str, raw: &str) -> Result<Value> {
        opaque(raw)?;
        let now = self.clock.now_ms();
        let rows=self.db.query(Statement::new("SELECT status,expires_at FROM oauth_requests WHERE kind='device' AND device_digest=? AND client_id=? AND resource=?",vec![digest(raw),client.into(),resource.into()])).await?;
        let row = rows.first().ok_or_else(|| oauth_error("invalid_grant"))?;
        if integer(row, "expires_at")? <= now {
            return Err(oauth_error("expired_token"));
        }
        match text(row, "status")?.as_str() {
            "denied" => return Err(oauth_error("access_denied")),
            "consumed" => return Err(oauth_error("invalid_grant")),
            _ => {}
        }
        let now = self.clock.now_ms();
        let nonce = random_id("devicepoll")?;
        let results=self.db.batch(vec![
            Statement::new("UPDATE oauth_requests SET interval_sec=interval_sec+5,next_poll_at=?+(interval_sec+5)*1000 WHERE kind='device' AND device_digest=? AND client_id=? AND resource=? AND expires_at>? AND status IN('pending','approved') AND next_poll_at>? RETURNING request_id",vec![now.into(),digest(raw),client.into(),resource.into(),now.into(),now.into()]),
            Statement::new("UPDATE oauth_requests SET next_poll_at=?+interval_sec*1000,claim_nonce=? WHERE kind='device' AND device_digest=? AND client_id=? AND resource=? AND expires_at>? AND status IN('pending','approved') AND next_poll_at<=? RETURNING status",vec![now.into(),nonce.clone().into(),digest(raw),client.into(),resource.into(),now.into(),now.into()])
        ]).await?;
        if !results[0].is_empty() {
            return Err(oauth_error("slow_down"));
        }
        let Some(row) = results[1].first() else {
            return Err(self.device_terminal_error(client, resource, raw).await?);
        };
        if text(row, "status")? == "pending" {
            return Err(oauth_error("authorization_pending"));
        }
        // Refresh time after awaiting polling and recheck authority inside issuance batch.
        let now = self.clock.now_ms();
        let (client_guard, client_args) = self.oauth_client_guard("q", now);
        let mut claim_args = vec![nonce.clone().into(), now.into()];
        claim_args.extend(client_args);
        let claim = Statement::new(
            format!(
                "UPDATE oauth_requests AS q SET status='consumed' WHERE claim_nonce=? AND status='approved' AND expires_at>? AND {LIVE_REQUEST} AND {client_guard}"
            ),
            claim_args,
        );
        match self.issue_request(vec![claim], &nonce, now).await {
            Err(error) if error.code == "invalid_grant" => {
                Err(self.device_terminal_error(client, resource, raw).await?)
            }
            result => result,
        }
    }
    // An await may span denial or expiry. Classify only after failed conditional
    // writes, keeping the usual polling path to its existing query/batch budget.
    async fn device_terminal_error(
        &self,
        client: &str,
        resource: &str,
        raw: &str,
    ) -> Result<Error> {
        let rows=self.db.query(Statement::new(format!("SELECT status,expires_at,({LIVE_REQUEST}) AS live FROM oauth_requests q WHERE kind='device' AND device_digest=? AND client_id=? AND resource=?"),vec![digest(raw),client.into(),resource.into()])).await?;
        let Some(row) = rows.first() else {
            return Ok(oauth_error("invalid_grant"));
        };
        if integer(row, "expires_at")? <= self.clock.now_ms() {
            return Ok(oauth_error("expired_token"));
        }
        Ok(oauth_error(if text(row, "status")? == "denied" {
            "access_denied"
        } else if matches!(text(row, "status")?.as_str(), "pending" | "approved")
            && (text(row, "status")? == "pending" || integer(row, "live")? == 1)
        {
            // Another interval-eligible poll can replace our nonce while this
            // request awaits D1. Keep the client polling; do not terminate a
            // still-live grant. The stored next_poll_at remains authoritative.
            "authorization_pending"
        } else {
            "invalid_grant"
        }))
    }
    pub(super) async fn oauth_revoke(&self, p: BTreeMap<String, String>) -> Result<Value> {
        fields(&p, &["client_id", "token", "token_type_hint"])?;
        let client = required(&p, "client_id")?;
        self.oauth_client(client).await?;
        let raw = required(&p, "token")?;
        // Unknown tokens return the same success; neither discovery nor oracle response.
        if raw.len() > 512 {
            return Ok(json!({}));
        }
        if self.keys.is_some() && self.keys_db.is_none() {
            self.db.batch(vec![Statement::new("UPDATE key_oauth_families SET revoked_at=COALESCE(revoked_at,?) WHERE client_id=? AND (family_id IN (SELECT family_id FROM key_oauth_refresh_tokens WHERE token_digest=?) OR family_id IN (SELECT family_id FROM key_access_tokens WHERE token_digest=?))",vec![self.clock.now_ms().into(),client.into(),digest(raw),digest(raw)])]).await?;
            return Ok(json!({}));
        }
        let now = self.clock.now_ms();
        let selector = "SELECT family_id FROM works_families WHERE client_id=? AND (family_id IN (SELECT family_id FROM works_refresh_tokens WHERE token_digest=?) OR family_id IN (SELECT family_id FROM works_access_tokens WHERE token_digest=?))";
        let args = vec![client.into(), digest(raw), digest(raw)];
        let mut q = vec![Statement::new(
            format!(
                "UPDATE works_families SET revoked_at=COALESCE(revoked_at,?) WHERE family_id IN ({selector})"
            ),
            {
                let mut a = vec![now.into()];
                a.extend(args.clone());
                a
            },
        )];
        q.extend(works_revoked_routes(selector, args, now));
        self.db.batch(q).await?;
        let selector = "SELECT f.family_id FROM oauth_families f WHERE f.client_id=? AND (EXISTS(SELECT 1 FROM oauth_refresh_tokens r WHERE r.family_id=f.family_id AND r.token_digest=?) OR EXISTS(SELECT 1 FROM oauth_access_tokens a JOIN tokens t ON t.token_id=a.token_id WHERE a.family_id=f.family_id AND t.token_digest=?))";
        self.db
            .batch(revoke_queries(
                selector,
                vec![client.into(), digest(raw), digest(raw)],
                self.clock.now_ms(),
            ))
            .await?;
        Ok(json!({}))
    }
}

#[cfg(all(test, feature = "native"))]
pub(super) mod tests {
    use super::*;
    use crate::native::{FileStore, SqliteDb};
    use std::cell::Cell;
    pub(super) struct TestClock(pub(super) Cell<i64>);
    impl Clock for TestClock {
        fn now_ms(&self) -> i64 {
            self.0.get()
        }
    }
    type TestApp = App<SqliteDb, FileStore>;
    const CODE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DEVICE: &str = "ddddddddddddddddddddddddddddddddddddddddddd";
    const VERIFIER: &str = "vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv";
    const RESOURCE: &str = "https://waypost.example/mcp";
    fn params(grant: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("grant_type".into(), grant.into()),
            ("client_id".into(), "test".into()),
            ("resource".into(), RESOURCE.into()),
        ])
    }
    pub(super) fn code_params() -> BTreeMap<String, String> {
        let mut p = params("authorization_code");
        p.extend([
            ("code".into(), CODE.into()),
            (
                "redirect_uri".into(),
                "https://client.example/callback".into(),
            ),
            ("code_verifier".into(), VERIFIER.into()),
        ]);
        p
    }
    pub(super) fn refresh_params(v: &Value) -> BTreeMap<String, String> {
        let mut p = params("refresh_token");
        p.insert(
            "refresh_token".into(),
            v["refresh_token"].as_str().unwrap().into(),
        );
        p
    }
    pub(super) fn device_params() -> BTreeMap<String, String> {
        let mut p = params("urn:ietf:params:oauth:grant-type:device_code");
        p.insert("device_code".into(), DEVICE.into());
        p
    }
    pub(super) async fn fixture() -> (tempfile::TempDir, TestApp, Rc<TestClock>) {
        let dir = tempfile::tempdir().unwrap();
        let clock = Rc::new(TestClock(Cell::new(1_000_000)));
        let mut app = App::new(
            SqliteDb::open(dir.path().join("db")).unwrap(),
            FileStore::open(dir.path().join("objects")).unwrap(),
            Config {
                admin_secret: "test-only-admin-secret-32-bytes-long".into(),
                public_origin: "https://waypost.example".into(),
            },
        )
        .with_clock(clock.clone());
        app.oidc=Some(serde_json::from_value(json!({"issuer":"https://fixture.example","client_id":"test","authorization_endpoint":"https://fixture.example/authorize","token_endpoint":"https://fixture.example/token","keys":[]})).unwrap());
        app = app
            .import_legacy_oauth_clients(super::super::oauth::OAuthConfig {
                clients: vec![
                    super::super::oauth::OAuthClient {
                        client_id: "test".into(),
                        name: "Test".into(),
                        redirect_uris: vec!["https://client.example/callback".into()],
                        allow_loopback_port_variation: false,
                    },
                    super::super::oauth::OAuthClient {
                        client_id: "other".into(),
                        name: "Other".into(),
                        redirect_uris: vec![],
                        allow_loopback_port_variation: false,
                    },
                ],
            })
            .await
            .unwrap();
        let response = app
            .handle(Request::json(
                "POST",
                "/admin/v0/spaces",
                &app.config.admin_secret,
                json!({"request_id":"oauth-test-space","display_name":"Test"}),
            ))
            .await;
        assert_eq!(response.status, 200);
        let space = serde_json::from_slice::<Value>(&response.body).unwrap()["data"].clone();
        let sid = space["space_id"].as_str().unwrap();
        let actor = space["default_actor_id"].as_str().unwrap();
        let consumer = space["default_consumer_id"].as_str().unwrap();
        app.db.batch(vec![
   Statement::new("INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES('acct','https://issuer.example','sub',?)",vec![clock.now_ms().into()]),
   Statement::new("INSERT INTO account_spaces(space_id,account_id,created_at) VALUES(?,'acct',?)",vec![sid.into(),clock.now_ms().into()]),
   Statement::new("INSERT INTO oauth_requests(request_id,kind,client_id,resource,scope,redirect_uri,code_challenge,code_digest,status,created_at,expires_at,next_poll_at,account_id,space_id,actor_id,consumer_id) VALUES('code','code','test',?,'waypost','https://client.example/callback',?,?,'approved',?,?,?,'acct',?,?,?)",vec![RESOURCE.into(),verifier(VERIFIER).unwrap().into(),digest(CODE),clock.now_ms().into(),(clock.now_ms()+600_000).into(),clock.now_ms().into(),sid.into(),actor.into(),consumer.into()]),
   Statement::new("INSERT INTO oauth_requests(request_id,kind,client_id,resource,scope,device_digest,status,created_at,expires_at,next_poll_at,account_id,space_id,actor_id,consumer_id) VALUES('device','device','test',?,'waypost',?,'pending',?,?,?,'acct',?,?,?)",vec![RESOURCE.into(),digest(DEVICE),clock.now_ms().into(),(clock.now_ms()+600_000).into(),clock.now_ms().into(),sid.into(),actor.into(),consumer.into()])
  ]).await.unwrap();
        (dir, app, clock)
    }
    async fn count(app: &TestApp, sql: &str) -> i64 {
        integer(
            &app.db.query(Statement::new(sql, vec![])).await.unwrap()[0],
            "n",
        )
        .unwrap()
    }
    #[tokio::test(flavor = "current_thread")]
    async fn approved_oauth_mount_is_bound_to_owner_and_current_policy() {
        for scenario in ["valid", "stale", "foreign", "paused"] {
            let (_dir, mut app, clock) = fixture().await;
            app.keys_db = Some(app.db.clone());
            app.db.batch(vec![
                Statement::new("UPDATE accounts SET issuer='https://fixture.example' WHERE account_id='acct'", vec![]),
                Statement::new("INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES('other','https://fixture.example','other',?)", vec![clock.now_ms().into()]),
                Statement::new("UPDATE oauth_requests SET scope='works:client',client_revision=0,space_id=NULL,actor_id=NULL,consumer_id=NULL WHERE request_id='code'", vec![]),
                Statement::new("INSERT INTO works_assistants(assistant_id,account_id,name,status,policy_revision,messages_enabled,created_at) VALUES('asst',?,'Assistant',?,?,1,?)", vec![if scenario == "foreign" {"other"} else {"acct"}.into(), if scenario == "paused" {"paused"} else {"active"}.into(), if scenario == "stale" {2} else {1}.into(),clock.now_ms().into()]),
                Statement::new("INSERT INTO works_oauth_mount_intents(request_id,account_id,assistant_id,policy_revision,created_at) VALUES('code','acct','asst',1,?)", vec![clock.now_ms().into()]),
            ]).await.unwrap();
            let result = app.oauth_token(code_params()).await;
            if scenario == "valid" {
                let issued = result.unwrap();
                let request = Request::json(
                    "POST",
                    "/mcp",
                    issued["access_token"].as_str().unwrap(),
                    json!({}),
                );
                assert!(
                    app.works_v2_authorize(&request, "asst")
                        .await
                        .unwrap()
                        .messages_enabled
                );
                assert_eq!(
                    count(
                        &app,
                        "SELECT count(*) AS n FROM works_attachments WHERE status='active'"
                    )
                    .await,
                    1
                );
            } else {
                assert_eq!(result.unwrap_err().code, "invalid_grant");
                assert_eq!(
                    count(&app, "SELECT count(*) AS n FROM works_attachments").await,
                    0
                );
                assert_eq!(
                    count(&app, "SELECT count(*) AS n FROM works_access_tokens").await,
                    0
                );
            }
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn works_bootstrap_pkce_mount_policy_and_refresh_replay() {
        let (_dir, mut app, clock) = fixture().await;
        app.keys_db = Some(app.db.clone());
        assert_eq!(app.normalize_scope(None).unwrap(), "works:client");
        assert_eq!(app.normalize_scope(Some("waypost")).unwrap(), "waypost");
        let mut challenge = Response::json(401, json!({}));
        app.oauth_challenge(&mut challenge);
        assert!(challenge.headers["www-authenticate"].contains("scope=\"works:client\""));
        app.db.batch(vec![
            Statement::new("UPDATE accounts SET issuer='https://fixture.example' WHERE account_id='acct'",vec![]),
            Statement::new("UPDATE oauth_requests SET scope='works:client',client_revision=0,space_id=NULL,actor_id=NULL,consumer_id=NULL WHERE request_id='code'",vec![]),
        ]).await.unwrap();
        let mut wrong = code_params();
        wrong.insert(
            "code_verifier".into(),
            "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".into(),
        );
        assert_eq!(
            app.oauth_token(wrong).await.unwrap_err().code,
            "invalid_grant"
        );
        let issued = app.oauth_token(code_params()).await.unwrap();
        assert_eq!(issued["scope"], "works:client");
        let request = Request::json(
            "POST",
            "/mcp",
            issued["access_token"].as_str().unwrap(),
            json!({}),
        );
        let b = app.works_v2_bootstrap(&request).await.unwrap();
        let install = text(&b, "installation_id").unwrap();
        assert_eq!(
            app.works_v2_authorize(&request, "asst")
                .await
                .unwrap_err()
                .code,
            "ASSISTANT_REQUIRED"
        );
        app.db.batch(vec![
            Statement::new("INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES('other','https://fixture.example','other',?)",vec![clock.now_ms().into()]),
            Statement::new("INSERT INTO works_assistants(assistant_id,account_id,name,created_at) VALUES('foreign','other','Foreign',?)",vec![clock.now_ms().into()]),
        ]).await.unwrap();
        assert!(
            app.works_v2_bootstrap_action(
                &request,
                "mount_request",
                json!({"assistant_id":"foreign"})
            )
            .await
            .is_err()
        );
        assert_eq!(
            count(
                &app,
                "SELECT count(*) AS n FROM works_attachments WHERE assistant_id='foreign'"
            )
            .await,
            0
        );
        app.db.batch(vec![Statement::new("INSERT INTO works_assistants(assistant_id,account_id,name,messages_enabled,created_at) VALUES('asst','acct','Assistant',1,?)",vec![clock.now_ms().into()])]).await.unwrap();
        let mount = app
            .works_v2_bootstrap_action(&request, "mount_request", json!({"assistant_id":"asst"}))
            .await
            .unwrap();
        assert_eq!(mount["status"], "approved");
        app.works_v2_bootstrap_action(&request, "mount_request", json!({"assistant_id":"asst"}))
            .await
            .unwrap();
        assert_eq!(count(&app,"SELECT count(*) AS n FROM works_attachments WHERE assistant_id='asst' AND status='active'").await,1);
        assert!(
            app.works_v2_authorize(&request, "asst")
                .await
                .unwrap()
                .messages_enabled
        );
        app.db.batch(vec![Statement::new("UPDATE works_assistants SET messages_enabled=0,policy_revision=policy_revision+1 WHERE assistant_id='asst'",vec![])]).await.unwrap();
        assert!(
            !app.works_v2_authorize(&request, "asst")
                .await
                .unwrap()
                .messages_enabled
        );
        assert_eq!(
            app.works_v2_bootstrap_action(
                &request,
                "mount_status",
                json!({"request_id":mount["request_id"]})
            )
            .await
            .unwrap()["status"],
            "approved"
        );
        app.db.batch(vec![
            Statement::new("INSERT INTO works_sessions(session_id,account_id,assistant_id,installation_id,host_kind,external_session_id,display_name,capability,created_at,last_seen_at) VALUES('replay_session','acct','asst',?,'test','ext','Test','pull',?,?)",vec![install.into(),clock.now_ms().into(),clock.now_ms().into()]),
            Statement::new("UPDATE works_assistants SET default_session_id='replay_session' WHERE assistant_id='asst'",vec![]),
            Statement::new("INSERT INTO works_deliveries(delivery_id,account_id,message_id,assistant_id,conversation_id,target_mode,assigned_session_id,state,created_at,updated_at) VALUES('replay_delivery','acct','m','asst','c','assistant','replay_session','claimed',?,?)",vec![clock.now_ms().into(),clock.now_ms().into()]),
        ]).await.unwrap();
        let next = app.oauth_token(refresh_params(&issued)).await.unwrap();
        assert_eq!(
            app.oauth_token(refresh_params(&issued))
                .await
                .unwrap_err()
                .code,
            "invalid_grant"
        );
        assert_eq!(count(&app,"SELECT count(*) n FROM works_attachments WHERE assistant_id='asst' AND status='revoked'").await,1);
        assert_eq!(count(&app,"SELECT count(*) n FROM works_deliveries WHERE delivery_id='replay_delivery' AND state='needs_handoff'").await,1);
        assert_eq!(count(&app,"SELECT count(*) n FROM works_assistants WHERE assistant_id='asst' AND default_session_id IS NULL").await,1);
        let new_request = Request::json(
            "POST",
            "/mcp",
            next["access_token"].as_str().unwrap(),
            json!({}),
        );
        assert_eq!(
            app.works_v2_bootstrap(&new_request).await.unwrap_err().code,
            "UNAUTHENTICATED"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn works_oauth_revoke_invalidates_mount_and_replaces_default() {
        let (_dir, mut app, clock) = fixture().await;
        app.keys_db = Some(app.db.clone());
        let now = clock.now_ms();
        app.db.batch(vec![
            Statement::new("UPDATE accounts SET issuer='https://fixture.example' WHERE account_id='acct'",vec![]),
            Statement::new("UPDATE oauth_requests SET scope='works:client',client_revision=0,space_id=NULL,actor_id=NULL,consumer_id=NULL WHERE request_id='code'",vec![]),
        ]).await.unwrap();
        let issued = app.oauth_token(code_params()).await.unwrap();
        let request = Request::json(
            "POST",
            "/mcp",
            issued["access_token"].as_str().unwrap(),
            json!({}),
        );
        let b = app.works_v2_bootstrap(&request).await.unwrap();
        let inst = text(&b, "installation_id").unwrap();
        app.db.batch(vec![
            Statement::new("INSERT INTO works_installations(installation_id,account_id,label,created_at) VALUES('other','acct','Other',?)",vec![now.into()]),
            Statement::new("INSERT INTO works_families(family_id,account_id,client_id,client_revision,resource,installation_id,created_at,expires_at) VALUES('other_family','acct','test',0,?,'other',?,?)",vec![RESOURCE.into(),now.into(),(now+1000000).into()]),
            Statement::new("INSERT INTO works_assistants(assistant_id,account_id,name,messages_enabled,default_session_id,created_at) VALUES('asst','acct','A',1,'s1',?)",vec![now.into()]),
            Statement::new("INSERT INTO works_attachments(attachment_id,account_id,assistant_id,installation_id,status,created_at) VALUES('a1','acct','asst',?,'active',?),('a2','acct','asst','other','active',?)",vec![inst.clone().into(),now.into(),now.into()]),
            Statement::new("INSERT INTO works_sessions(session_id,account_id,assistant_id,installation_id,host_kind,external_session_id,display_name,capability,created_at,last_seen_at) VALUES('s1','acct','asst',?,'test','ext1','A','pull',?,?),('s2','acct','asst','other','test','ext2','B','pull',?,?)",vec![inst.into(),now.into(),now.into(),now.into(),now.into()]),
            Statement::new("INSERT INTO works_deliveries(delivery_id,account_id,message_id,assistant_id,conversation_id,target_mode,assigned_session_id,state,created_at,updated_at) VALUES('d','acct','m','asst','c','assistant','s1','assigned',?,?)",vec![now.into(),now.into()]),
        ]).await.unwrap();
        app.oauth_revoke(BTreeMap::from([
            ("client_id".into(), "test".into()),
            (
                "token".into(),
                issued["access_token"].as_str().unwrap().into(),
            ),
        ]))
        .await
        .unwrap();
        assert_eq!(count(&app,"SELECT count(*) n FROM works_attachments WHERE attachment_id='a1' AND status='revoked'").await,1);
        assert_eq!(count(&app,"SELECT count(*) n FROM works_attachments WHERE attachment_id='a2' AND status='active'").await,1);
        assert_eq!(count(&app,"SELECT count(*) n FROM works_deliveries WHERE delivery_id='d' AND state='needs_handoff' AND assignment_generation=2").await,1);
        assert_eq!(count(&app,"SELECT count(*) n FROM works_assistants WHERE assistant_id='asst' AND default_session_id='s2'").await,1);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn works_mount_approval_checks_session_revision_and_is_one_use() {
        let (_dir, mut app, clock) = fixture().await;
        app.keys_db = Some(app.db.clone());
        let now = clock.now_ms();
        let cookie = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        app.db.batch(vec![
            Statement::new("UPDATE accounts SET issuer='https://fixture.example' WHERE account_id='acct'",vec![]),
            Statement::new("INSERT INTO account_sessions(session_digest,account_id,created_at,expires_at) VALUES(?,'acct',?,?)",vec![digest(cookie),now.into(),(now+3600000).into()]),
            Statement::new("INSERT INTO works_installations(installation_id,account_id,label,created_at) VALUES('inst','acct','Test',?)",vec![now.into()]),
            Statement::new("INSERT INTO works_families(family_id,account_id,client_id,client_revision,resource,installation_id,created_at,expires_at) VALUES('fam','acct','test',0,?,'inst',?,?)",vec![RESOURCE.into(),now.into(),(now+3600000).into()]),
            Statement::new("INSERT INTO works_assistants(assistant_id,account_id,name,created_at) VALUES('asst','acct','Test',?)",vec![now.into()]),
            Statement::new("INSERT INTO works_mount_requests(request_id,account_id,installation_id,family_id,assistant_id,policy_revision,status,created_at,expires_at) VALUES('mount','acct','inst','fam','asst',1,'pending',?,?)",vec![now.into(),(now+600000).into()])]).await.unwrap();
        let mut req = Request::json(
            "POST",
            "/account/works/mount-requests/approve",
            "",
            json!({"request_id":"mount","policy_revision":1}),
        );
        req.headers
            .insert("cookie".into(), format!("__Host-waypost_account={cookie}"));
        req.headers
            .insert("origin".into(), app.config.public_origin.clone());
        req.headers
            .insert("x-waypost-csrf".into(), app.csrf_token(cookie).unwrap());
        let mut no_csrf = req.clone();
        no_csrf.headers.remove("x-waypost-csrf");
        assert_eq!(app.works_v2_account_handle(&no_csrf).await.status, 403);
        app.db
            .batch(vec![Statement::new(
                "UPDATE works_assistants SET policy_revision=2 WHERE assistant_id='asst'",
                vec![],
            )])
            .await
            .unwrap();
        assert_eq!(app.works_v2_account_handle(&req).await.status, 409);
        let mut review = req.clone();
        review.path = "/account/works/mount-requests/review".into();
        review.body = serde_json::to_vec(&json!({"request_id":"mount"})).unwrap();
        let response = app.works_v2_account_handle(&review).await;
        assert_eq!(response.status, 200);
        let reviewed: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(reviewed["data"]["policy_revision"], 2);
        assert_eq!(count(&app,"SELECT count(*) n FROM works_mount_requests WHERE request_id='mount' AND status='denied' AND replaced_by IS NOT NULL").await,1);
        req.body = serde_json::to_vec(
            &json!({"request_id":reviewed["data"]["request_id"],"policy_revision":2}),
        )
        .unwrap();
        let approved = app.works_v2_account_handle(&req).await;
        assert_eq!(approved.status, 200);
        let approval: Value = serde_json::from_slice(&approved.body).unwrap();
        assert_eq!(app.works_v2_account_handle(&req).await.status, 409);
        assert_eq!(
            count(
                &app,
                "SELECT count(*) n FROM works_attachments WHERE status='active'"
            )
            .await,
            1
        );
        app.db.batch(vec![
            Statement::new("INSERT INTO works_sessions(session_id,account_id,assistant_id,installation_id,host_kind,external_session_id,display_name,capability,created_at,last_seen_at) VALUES('session','acct','asst','inst','test','external','Test','pull',?,?)",vec![now.into(),now.into()]),
            Statement::new("UPDATE works_assistants SET default_session_id='session' WHERE assistant_id='asst'",vec![]),
            Statement::new("INSERT INTO works_deliveries(delivery_id,account_id,message_id,assistant_id,conversation_id,target_mode,assigned_session_id,state,created_at,updated_at) VALUES('d','acct','m','asst','c','assistant','session','assigned',?,?)",vec![now.into(),now.into()]),
        ]).await.unwrap();
        req.path = "/account/works/attachments/revoke".into();
        req.body = serde_json::to_vec(&json!({"attachment_id":approval["data"]["attachment_id"]}))
            .unwrap();
        assert_ne!(app.works_v2_account_handle(&req).await.status, 200);
        assert_eq!(count(&app,"SELECT count(*) n FROM works_deliveries WHERE delivery_id='d' AND state='assigned' AND assignment_generation=1").await,1);
        assert_eq!(count(&app,"SELECT count(*) n FROM works_assistants WHERE assistant_id='asst' AND default_session_id='session'").await,1);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn code_binding_and_replay_revoke_family() {
        let (_dir, app, _clock) = fixture().await;
        for (key, value, error) in [
            (
                "code_verifier",
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                "invalid_grant",
            ),
            (
                "redirect_uri",
                "https://evil.example/callback",
                "invalid_grant",
            ),
            ("client_id", "other", "invalid_grant"),
            ("resource", "https://other.example/mcp", "invalid_target"),
            ("scope", "admin", "invalid_scope"),
        ] {
            let mut p = code_params();
            p.insert(key.into(), value.into());
            assert_eq!(app.oauth_token(p).await.unwrap_err().code, error);
        }
        let result = app.oauth_token(code_params()).await.unwrap();
        assert_eq!(result["expires_in"], 300);
        let mut wrong_replay = code_params();
        wrong_replay.insert(
            "code_verifier".into(),
            "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".into(),
        );
        assert_eq!(
            app.oauth_token(wrong_replay).await.unwrap_err().code,
            "invalid_grant"
        );
        assert_eq!(
            count(
                &app,
                "SELECT count(*) n FROM oauth_families WHERE revoked_at IS NULL"
            )
            .await,
            1
        );
        assert!(result["access_token"].as_str().unwrap().starts_with("wpo_"));
        assert_eq!(
            count(
                &app,
                "SELECT count(*) n FROM tokens WHERE credential_kind='oauth' AND revoked_at IS NULL"
            )
            .await,
            1
        );
        assert_eq!(
            app.oauth_token(code_params()).await.unwrap_err().code,
            "invalid_grant"
        );
        assert_eq!(
            count(
                &app,
                "SELECT count(*) n FROM oauth_families WHERE revoked_at IS NULL"
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &app,
                "SELECT count(*) n FROM tokens WHERE credential_kind='oauth' AND revoked_at IS NULL"
            )
            .await,
            0
        );
        assert_eq!(
            app.oauth_token(refresh_params(&result))
                .await
                .unwrap_err()
                .code,
            "invalid_grant"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn refresh_rotation_wrong_client_and_replay() {
        let (_dir, app, clock) = fixture().await;
        let first = app.oauth_token(code_params()).await.unwrap();
        clock.0.set(clock.now_ms() + 1000);
        let mut wrong = refresh_params(&first);
        wrong.insert("client_id".into(), "other".into());
        assert_eq!(
            app.oauth_token(wrong).await.unwrap_err().code,
            "invalid_grant"
        );
        let second = app.oauth_token(refresh_params(&first)).await.unwrap();
        assert_ne!(first["refresh_token"], second["refresh_token"]);
        assert_eq!(
            count(
                &app,
                "SELECT count(*) n FROM oauth_refresh_tokens WHERE used_at IS NOT NULL"
            )
            .await,
            1
        );
        assert_eq!(
            app.oauth_token(refresh_params(&first))
                .await
                .unwrap_err()
                .code,
            "invalid_grant"
        );
        assert_eq!(
            app.oauth_token(refresh_params(&second))
                .await
                .unwrap_err()
                .code,
            "invalid_grant"
        );
        assert_eq!(
            count(
                &app,
                "SELECT count(*) n FROM tokens WHERE credential_kind='oauth' AND revoked_at IS NULL"
            )
            .await,
            0
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn device_polling_backoff_and_one_use() {
        let (_dir, app, clock) = fixture().await;
        assert_eq!(
            app.oauth_token(device_params()).await.unwrap_err().code,
            "authorization_pending"
        );
        assert_eq!(
            app.oauth_token(device_params()).await.unwrap_err().code,
            "slow_down"
        );
        assert_eq!(
            app.oauth_token(device_params()).await.unwrap_err().code,
            "slow_down"
        );
        assert_eq!(
            count(
                &app,
                "SELECT interval_sec n FROM oauth_requests WHERE request_id='device'"
            )
            .await,
            15
        );
        app.db
            .batch(vec![Statement::new(
                "UPDATE oauth_requests SET status='approved' WHERE request_id='device'",
                vec![],
            )])
            .await
            .unwrap();
        clock.0.set(clock.now_ms() + 15_000);
        let tokens = app.oauth_token(device_params()).await.unwrap();
        assert_eq!(tokens["token_type"], "Bearer");
        assert_eq!(
            app.oauth_token(device_params()).await.unwrap_err().code,
            "invalid_grant"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn authority_revocation_expiry_and_device_denial() {
        let (_dir, app, clock) = fixture().await;
        app.db
            .batch(vec![Statement::new(
                "UPDATE oauth_requests SET status='denied' WHERE request_id='device'",
                vec![],
            )])
            .await
            .unwrap();
        assert_eq!(
            app.oauth_token(device_params()).await.unwrap_err().code,
            "access_denied"
        );
        let issued = app.oauth_token(code_params()).await.unwrap();
        app.db
            .batch(vec![Statement::new(
                "DELETE FROM account_spaces WHERE account_id='acct'",
                vec![],
            )])
            .await
            .unwrap();
        assert_eq!(
            app.oauth_token(refresh_params(&issued))
                .await
                .unwrap_err()
                .code,
            "invalid_grant"
        );
        clock.0.set(clock.now_ms() + 600_001);
        assert_eq!(
            app.oauth_token(device_params()).await.unwrap_err().code,
            "expired_token"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn revocation_ownership_unknown_and_access_or_refresh() {
        for key in ["access_token", "refresh_token"] {
            let (_dir, app, _clock) = fixture().await;
            let issued = app.oauth_token(code_params()).await.unwrap();
            let revoke = |client: &str, token: &str| {
                BTreeMap::from([
                    ("client_id".into(), client.into()),
                    ("token".into(), token.into()),
                ])
            };
            assert_eq!(
                app.oauth_revoke(revoke("test", "unknown")).await.unwrap(),
                json!({})
            );
            assert_eq!(
                app.oauth_revoke(revoke("other", issued[key].as_str().unwrap()))
                    .await
                    .unwrap(),
                json!({})
            );
            assert_eq!(
                count(
                    &app,
                    "SELECT count(*) n FROM oauth_families WHERE revoked_at IS NULL"
                )
                .await,
                1
            );
            app.oauth_revoke(revoke("test", issued[key].as_str().unwrap()))
                .await
                .unwrap();
            assert_eq!(
                app.oauth_token(refresh_params(&issued))
                    .await
                    .unwrap_err()
                    .code,
                "invalid_grant"
            );
            assert_eq!(count(&app,"SELECT count(*) n FROM tokens WHERE credential_kind='oauth' AND revoked_at IS NULL").await,0);
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn code_and_refresh_expiry_never_create_extra_tokens() {
        let (_dir, app, clock) = fixture().await;
        clock.0.set(clock.now_ms() + 600_001);
        assert_eq!(
            app.oauth_token(code_params()).await.unwrap_err().code,
            "invalid_grant"
        );
        assert_eq!(
            count(&app, "SELECT count(*) n FROM oauth_families").await,
            0
        );
        let (_dir, app, clock) = fixture().await;
        let first = app.oauth_token(code_params()).await.unwrap();
        clock.0.set(clock.now_ms() + FAMILY_MS - 1000);
        let last = app.oauth_token(refresh_params(&first)).await.unwrap();
        assert_eq!(last["expires_in"], 1);
        clock.0.set(clock.now_ms() + 1000);
        assert_eq!(
            app.oauth_token(refresh_params(&last))
                .await
                .unwrap_err()
                .code,
            "invalid_grant"
        );
        assert_eq!(
            count(&app, "SELECT count(*) n FROM oauth_access_tokens").await,
            2
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn disabled_identity_cannot_redeem_approved_code_or_device() {
        for statement in [
            "UPDATE accounts SET disabled=1",
            "UPDATE actors SET disabled=1",
            "UPDATE consumers SET disabled=1",
            "DELETE FROM account_spaces",
        ] {
            let (_dir, app, _clock) = fixture().await;
            app.db
                .batch(vec![
                    Statement::new(statement, vec![]),
                    Statement::new(
                        "UPDATE oauth_requests SET status='approved' WHERE request_id='device'",
                        vec![],
                    ),
                ])
                .await
                .unwrap();
            assert_eq!(
                app.oauth_token(code_params()).await.unwrap_err().code,
                "invalid_grant"
            );
            assert_eq!(
                app.oauth_token(device_params()).await.unwrap_err().code,
                "invalid_grant"
            );
            assert_eq!(
                count(&app, "SELECT count(*) n FROM oauth_families").await,
                0
            );
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn access_authentication_and_final_guard_fail_closed() {
        for statement in [
            "UPDATE oauth_families SET revoked_at=1",
            "UPDATE oauth_families SET resource='https://other.example/mcp'",
            "UPDATE oauth_families SET client_id='removed'",
            "UPDATE accounts SET disabled=1",
            "DELETE FROM account_spaces",
            "DELETE FROM oauth_access_tokens",
            "UPDATE oauth_families SET expires_at=1",
        ] {
            let (_dir, app, _clock) = fixture().await;
            let issued = app.oauth_token(code_params()).await.unwrap();
            let request = Request::json(
                "POST",
                "/mcp",
                issued["access_token"].as_str().unwrap(),
                json!({}),
            );
            let principal = app.authenticate(&request).await.unwrap();
            app.current(&principal).await.unwrap();
            app.db
                .batch(vec![Statement::new(statement, vec![])])
                .await
                .unwrap();
            assert!(app.authenticate(&request).await.is_err(), "{statement}");
            assert!(app.current(&principal).await.is_err(), "{statement}");
        }
        let (_dir, mut app, _clock) = fixture().await;
        let issued = app.oauth_token(code_params()).await.unwrap();
        let request = Request::json(
            "POST",
            "/mcp",
            issued["access_token"].as_str().unwrap(),
            json!({}),
        );
        let principal = app.authenticate(&request).await.unwrap();
        app.oauth_discovery = None;
        assert!(app.authenticate(&request).await.is_err());
        assert!(app.current(&principal).await.is_err());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn mcp_unauthorized_has_resource_challenge_when_enabled() {
        let (_dir, app, _clock) = fixture().await;
        let response = app
            .handle(Request::json("POST", "/mcp", "invalid", json!({})))
            .await;
        assert_eq!(response.status, 401);
        assert_eq!(
            response.headers.get("www-authenticate").unwrap(),
            "Bearer resource_metadata=\"https://waypost.example/.well-known/oauth-protected-resource/mcp\", scope=\"waypost\""
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn removed_redirect_cannot_exchange_already_approved_code() {
        let (_dir, app, _clock) = fixture().await;
        app.db.batch(vec![Statement::new("UPDATE oauth_dynamic_clients SET metadata_json=json_set(metadata_json,'$.redirect_uris',json('[]')) WHERE client_id='test'",vec![])]).await.unwrap();
        assert_eq!(
            app.oauth_token(code_params()).await.unwrap_err().code,
            "invalid_grant"
        );
        assert_eq!(
            count(&app, "SELECT count(*) n FROM oauth_families").await,
            0
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn device_expiry_during_poll_or_claim_returns_expired_token() {
        struct AdvancingClock {
            now: Cell<i64>,
            step: i64,
        }
        impl Clock for AdvancingClock {
            fn now_ms(&self) -> i64 {
                let now = self.now.get();
                self.now.set(now + self.step);
                now
            }
        }
        for step in [600_001, 300_001] {
            let (_dir, mut app, _clock) = fixture().await;
            app.db
                .batch(vec![Statement::new(
                    "UPDATE oauth_requests SET status='approved' WHERE request_id='device'",
                    vec![],
                )])
                .await
                .unwrap();
            app.clock = Rc::new(AdvancingClock {
                now: Cell::new(1_000_000),
                step,
            });
            assert_eq!(
                app.oauth_token(device_params()).await.unwrap_err().code,
                "expired_token"
            );
            assert_eq!(
                count(&app, "SELECT count(*) n FROM oauth_families").await,
                0
            );
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn device_denial_between_read_poll_and_claim_is_reported() {
        struct DenialClock {
            path: std::path::PathBuf,
            calls: Cell<usize>,
            deny_on: usize,
        }
        impl Clock for DenialClock {
            fn now_ms(&self) -> i64 {
                let calls = self.calls.get() + 1;
                self.calls.set(calls);
                if calls == self.deny_on {
                    rusqlite::Connection::open(&self.path)
                        .unwrap()
                        .execute(
                            "UPDATE oauth_requests SET status='denied' WHERE request_id='device'",
                            [],
                        )
                        .unwrap();
                }
                1_000_000
            }
        }
        for deny_on in [2, 3] {
            let (dir, mut app, _clock) = fixture().await;
            app.db
                .batch(vec![Statement::new(
                    "UPDATE oauth_requests SET status='approved' WHERE request_id='device'",
                    vec![],
                )])
                .await
                .unwrap();
            app.clock = Rc::new(DenialClock {
                path: dir.path().join("db"),
                calls: Cell::new(0),
                deny_on,
            });
            assert_eq!(
                app.oauth_token(device_params()).await.unwrap_err().code,
                "access_denied"
            );
            assert_eq!(
                count(&app, "SELECT count(*) n FROM oauth_families").await,
                0
            );
        }
    }
}

#[cfg(all(test, feature = "native"))]
#[path = "oauth_token_adversarial_tests.rs"]
mod adversarial_tests;
