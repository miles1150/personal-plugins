//! Account-scoped work sessions and fenced message execution.
use super::works_v2::WorksPrincipal;
use super::*;
use crate::storage::driver::optional_text;

fn s<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::invalid("Missing or invalid field."))
}
fn number(v: &Value, key: &str) -> Result<i64> {
    v.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| Error::invalid("Missing or invalid generation."))
}
fn rows_json(rows: Vec<Row>, fields: &[&str]) -> Value {
    Value::Array(
        rows.into_iter()
            .map(|r| {
                let mut m = serde_json::Map::new();
                for &field in fields {
                    let v = match r.get(field) {
                        Some(Sql::Text(s)) => json!(s),
                        Some(Sql::Integer(i)) => json!(i),
                        _ => Value::Null,
                    };
                    m.insert(field.into(), v);
                }
                Value::Object(m)
            })
            .collect(),
    )
}
impl<D: Database, O: ObjectStore> App<D, O> {
    pub(super) async fn works_sessions_dispatch(
        &self,
        p: &WorksPrincipal,
        operation: &str,
        v: Value,
    ) -> Result<Value> {
        if !p.messages_enabled {
            return Err(Error::new(
                "FORBIDDEN",
                "Messages are disabled for this assistant.",
            ));
        }
        match operation {
            "sessions_register" => self.works_register(p, v).await,
            "sessions_list" => self.works_list(p).await,
            "sessions_update" => self.works_update(p, v).await,
            "recipients_list" => self.works_recipients(p).await,
            "messages_send" => self.works_send(p, v).await,
            "messages_read" => self.works_read(p, v).await,
            "deliveries_claim" => self.works_claim(p, v).await,
            "deliveries_renew" => self.works_finish(p, v, true).await,
            "deliveries_complete" => self.works_finish(p, v, false).await,
            _ => Err(Error::not_found()),
        }
    }
    async fn works_register(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        let host = s(&v, "host_kind")?;
        let external = s(&v, "external_session_id")?;
        let name = s(&v, "display_name")?;
        if host.len() > 64 || external.len() > 512 || name.len() > 200 {
            return Err(Error::invalid("Session field too long."));
        }
        let cap = v
            .get("capability")
            .and_then(Value::as_str)
            .unwrap_or("pull");
        if !["pull", "wakeable"].contains(&cap) {
            return Err(Error::invalid("Invalid session capability."));
        }
        let visible = v
            .get("discoverable")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let id = random_id("ses")?;
        let now = self.clock.now_ms();
        let (g, ga) = self.works_v2_guard(p);
        let mut args = vec![
            id.clone().into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            p.installation_id.clone().into(),
            host.into(),
            external.into(),
            name.into(),
            cap.into(),
            visible.into(),
            now.into(),
            now.into(),
        ];
        args.extend(ga);
        let result=self.db.batch(vec![Statement::new(format!("INSERT INTO works_sessions(session_id,account_id,assistant_id,installation_id,host_kind,external_session_id,display_name,capability,discoverable,created_at,last_seen_at) SELECT ?,?,?,?,?,?,?,?,?,?,? WHERE {g} ON CONFLICT(installation_id,host_kind,external_session_id) WHERE status <> 'deleted' DO UPDATE SET last_seen_at=excluded.last_seen_at,display_name=excluded.display_name,capability=excluded.capability,discoverable=excluded.discoverable WHERE works_sessions.account_id=excluded.account_id AND works_sessions.assistant_id=excluded.assistant_id AND works_sessions.status<>'deleted' RETURNING session_id"),args)]).await?;
        let row = result[0]
            .first()
            .ok_or_else(|| Error::new("REVISION_CONFLICT", "Session registration was rejected."))?;
        Ok(json!({"session_id":text(row,"session_id")?}))
    }
    async fn works_check_current(&self, p: &WorksPrincipal) -> Result<()> {
        let (g, a) = self.works_v2_guard(p);
        let rows = self
            .db
            .query(Statement::new(format!("SELECT 1 AS allowed WHERE {g}"), a))
            .await?;
        if rows.is_empty() {
            Err(Error::unauthenticated())
        } else {
            Ok(())
        }
    }
    async fn works_list(&self, p: &WorksPrincipal) -> Result<Value> {
        let (g, ga) = self.works_v2_guard(p);
        let mut args = vec![p.account_id.clone().into(), p.assistant_id.clone().into()];
        args.extend(ga);
        let rows=self.db.query(Statement::new(format!("SELECT session_id,display_name,host_kind,capability,discoverable,status,last_seen_at,installation_id,route_revision FROM works_sessions WHERE account_id=? AND assistant_id=? AND {g} ORDER BY created_at,session_id"),args)).await?;
        self.works_check_current(p).await?;
        Ok(
            json!({"sessions":rows_json(rows,&["session_id","display_name","host_kind","capability","discoverable","status","last_seen_at","installation_id","route_revision"])}),
        )
    }
    async fn works_recipients(&self, p: &WorksPrincipal) -> Result<Value> {
        let (g, ga) = self.works_v2_guard(p);
        let mut args = vec![p.account_id.clone().into()];
        args.extend(ga.clone());
        let rows=self.db.query(Statement::new(format!("SELECT assistant_id,name,default_session_id FROM works_assistants WHERE account_id=? AND status='active' AND messages_enabled=1 AND {g} ORDER BY name,assistant_id"),args)).await?;
        let mut args = vec![p.account_id.clone().into()];
        args.extend(ga);
        let sessions=self.db.query(Statement::new(format!("SELECT s.assistant_id,s.session_id,s.display_name,s.host_kind,s.capability,s.status FROM works_sessions s JOIN works_installations i ON i.installation_id=s.installation_id AND i.account_id=s.account_id JOIN works_attachments t ON t.installation_id=s.installation_id AND t.assistant_id=s.assistant_id AND t.account_id=s.account_id WHERE s.account_id=? AND s.discoverable=1 AND s.status='registered' AND i.status='active' AND t.status='active' AND {g} ORDER BY s.created_at,s.session_id"),args)).await?;
        self.works_check_current(p).await?;
        Ok(
            json!({"assistants":rows_json(rows,&["assistant_id","name","default_session_id"]),"sessions":rows_json(sessions,&["assistant_id","session_id","display_name","host_kind","capability","status"])}),
        )
    }
    async fn works_update(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        if let Some(default) = v.get("default_session_id") {
            if v.get("session_id").is_some() || !(default.is_null() || default.is_string()) {
                return Err(Error::invalid("Set the default in a separate update."));
            }
            let dest = default.as_str().map(str::to_owned);
            let now = self.clock.now_ms();
            let (g, ga) = self.works_v2_guard(p);
            let mut args = vec![
                dest.clone().into(),
                p.account_id.clone().into(),
                p.assistant_id.clone().into(),
                dest.clone().into(),
                dest.clone().into(),
            ];
            args.extend(ga.clone());
            let mut updates = vec![Statement::new(
                format!(
                    "UPDATE works_assistants SET default_session_id=?,route_revision=route_revision+1 WHERE account_id=? AND assistant_id=? AND (? IS NULL OR EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=works_assistants.account_id AND s.assistant_id=works_assistants.assistant_id AND s.session_id=? AND s.status='registered' AND i.status='active' AND t.status='active')) AND {g} RETURNING assistant_id"
                ),
                args,
            )];
            if let Some(id) = dest.clone() {
                let mut args = vec![
                    id.clone().into(),
                    now.into(),
                    p.account_id.clone().into(),
                    p.assistant_id.clone().into(),
                    id.into(),
                ];
                args.extend(ga);
                updates.push(Statement::new(format!("UPDATE works_deliveries SET assigned_session_id=COALESCE((SELECT c.owner_session_id FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id),?),state=CASE WHEN EXISTS(SELECT 1 FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id AND c.owner_session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.session_id=c.owner_session_id AND s.status='registered' AND i.status='active' AND t.status='active')) THEN 'needs_handoff' ELSE 'assigned' END,assignment_generation=assignment_generation+1,updated_at=? WHERE account_id=? AND assistant_id=? AND state='waiting_for_session' AND target_mode='assistant' AND EXISTS(SELECT 1 FROM works_assistants a WHERE a.account_id=works_deliveries.account_id AND a.assistant_id=works_deliveries.assistant_id AND a.default_session_id=?) AND {g}"),args));
            }
            let result = self.db.batch(updates).await?;
            if result[0].is_empty() {
                return Err(Error::new(
                    "REVISION_CONFLICT",
                    "Default session is unavailable.",
                ));
            }
            return Ok(json!({"default_session_id":dest}));
        }
        let id = s(&v, "session_id")?;
        if v.get("status").is_none() && v.get("set_default").is_none() {
            let visible = v.get("discoverable").and_then(Value::as_bool);
            let name = v.get("display_name").and_then(Value::as_str);
            if visible.is_none() && name.is_none() {
                return Err(Error::invalid("No session update supplied."));
            }
            if name.is_some_and(|s| s.is_empty() || s.len() > 200) {
                return Err(Error::invalid("Invalid display name."));
            }
            let (g, ga) = self.works_v2_guard(p);
            let mut args = vec![
                visible.map(i64::from).into(),
                name.map(str::to_owned).into(),
                p.account_id.clone().into(),
                p.assistant_id.clone().into(),
                p.installation_id.clone().into(),
                id.into(),
            ];
            args.extend(ga);
            let result=self.db.batch(vec![Statement::new(format!("UPDATE works_sessions SET discoverable=COALESCE(?,discoverable),display_name=COALESCE(?,display_name) WHERE account_id=? AND assistant_id=? AND installation_id=? AND session_id=? AND status<>'deleted' AND {g} RETURNING session_id"),args)]).await?;
            if result[0].is_empty() {
                return Err(Error::new("REVISION_CONFLICT", "Session update rejected."));
            }
            return Ok(json!({"session_id":id}));
        }
        let existing_status = if v.get("status").is_none() {
            let rows=self.db.query(Statement::new("SELECT status FROM works_sessions WHERE account_id=? AND assistant_id=? AND installation_id=? AND session_id=? AND status<>'deleted'",vec![p.account_id.clone().into(),p.assistant_id.clone().into(),p.installation_id.clone().into(),id.into()])).await?;
            Some(text(rows.first().ok_or_else(Error::not_found)?, "status")?)
        } else {
            None
        };
        let status = v
            .get("status")
            .and_then(Value::as_str)
            .or(existing_status.as_deref())
            .unwrap_or("registered");
        if !["registered", "paused", "deleted"].contains(&status) {
            return Err(Error::invalid("Invalid session status."));
        }
        let now = self.clock.now_ms();
        let (g, ga) = self.works_v2_guard(p);
        let mut args = vec![
            status.into(),
            now.into(),
            status.into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            p.installation_id.clone().into(),
            id.into(),
        ];
        args.extend(ga.clone());
        let mut statements = vec![Statement::new(
            format!(
                "UPDATE works_sessions SET status=?,last_seen_at=?,route_revision=route_revision+CASE WHEN status<>? THEN 1 ELSE 0 END WHERE account_id=? AND assistant_id=? AND installation_id=? AND session_id=? AND status<>'deleted' AND {g} RETURNING session_id"
            ),
            args,
        )];
        if status == "deleted" {
            let mut a = vec![
                now.into(),
                p.account_id.clone().into(),
                p.assistant_id.clone().into(),
                id.into(),
            ];
            a.extend(ga.clone());
            statements.push(Statement::new(format!("UPDATE works_deliveries SET state='needs_handoff',assignment_generation=assignment_generation+1,updated_at=? WHERE account_id=? AND assistant_id=? AND assigned_session_id=? AND state IN ('assigned','claimed','unknown') AND {g}"),a));
            let mut a = vec![
                now.into(),
                p.account_id.clone().into(),
                p.assistant_id.clone().into(),
                id.into(),
            ];
            a.extend(ga.clone());
            statements.push(Statement::new(format!("UPDATE works_assistants SET default_session_id=(SELECT s.session_id FROM works_sessions s JOIN works_installations i ON i.installation_id=s.installation_id AND i.account_id=s.account_id JOIN works_attachments t ON t.installation_id=s.installation_id AND t.assistant_id=s.assistant_id AND t.account_id=s.account_id WHERE s.account_id=works_assistants.account_id AND s.assistant_id=works_assistants.assistant_id AND s.status='registered' AND i.status='active' AND t.status='active' ORDER BY CASE WHEN s.capability='wakeable' AND s.last_seen_at>=? - 120000 THEN 0 ELSE 1 END,s.created_at,s.session_id LIMIT 1),route_revision=route_revision+1 WHERE account_id=? AND assistant_id=? AND default_session_id=? AND {g}"),a));
        }
        if v.get("set_default").and_then(Value::as_bool) == Some(true) && status == "registered" {
            let mut a = vec![
                id.into(),
                p.account_id.clone().into(),
                p.assistant_id.clone().into(),
                id.into(),
            ];
            a.extend(ga.clone());
            statements.push(Statement::new(format!("UPDATE works_assistants SET default_session_id=?,route_revision=route_revision+1 WHERE account_id=? AND assistant_id=? AND EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=works_assistants.account_id AND s.assistant_id=works_assistants.assistant_id AND s.session_id=? AND s.status='registered' AND i.status='active' AND t.status='active') AND {g}"),a));
            let mut a = vec![
                id.into(),
                now.into(),
                p.account_id.clone().into(),
                p.assistant_id.clone().into(),
                id.into(),
            ];
            a.extend(ga);
            statements.push(Statement::new(format!("UPDATE works_deliveries SET assigned_session_id=COALESCE((SELECT c.owner_session_id FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id),?),state=CASE WHEN EXISTS(SELECT 1 FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id AND c.owner_session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.session_id=c.owner_session_id AND s.status='registered' AND i.status='active' AND t.status='active')) THEN 'needs_handoff' ELSE 'assigned' END,assignment_generation=assignment_generation+1,updated_at=? WHERE account_id=? AND assistant_id=? AND state='waiting_for_session' AND target_mode='assistant' AND EXISTS(SELECT 1 FROM works_assistants WHERE account_id=works_deliveries.account_id AND assistant_id=works_deliveries.assistant_id AND default_session_id=?) AND {g}"),a));
        }
        let result = self.db.batch(statements).await?;
        if result[0].is_empty() {
            return Err(Error::new("REVISION_CONFLICT", "Session update rejected."));
        }
        let rows=self.db.query(Statement::new("SELECT status FROM works_sessions WHERE account_id=? AND assistant_id=? AND installation_id=? AND session_id=?",vec![p.account_id.clone().into(),p.assistant_id.clone().into(),p.installation_id.clone().into(),id.into()])).await?;
        if rows.first().map(|r| text(r, "status").ok()) != Some(Some(status.into())) {
            return Err(Error::new("REVISION_CONFLICT", "Session update rejected."));
        }
        Ok(json!({"session_id":id,"status":status}))
    }
    async fn works_send(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        let (g, ga) = self.works_v2_guard(p);
        self.works_send_guard(p, v, g, ga).await
    }
    async fn works_send_guard(
        &self,
        p: &WorksPrincipal,
        v: Value,
        g: String,
        ga: Vec<Sql>,
    ) -> Result<Value> {
        let attachments = match v.get("attachments") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(xs)) => xs
                .iter()
                .map(|x| {
                    x.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| Error::invalid("Invalid attachment reference."))
                })
                .collect::<Result<Vec<_>>>()?,
            _ => return Err(Error::invalid("Invalid attachments.")),
        };
        if attachments.len() > self.limits.max_message_attachments {
            return Err(Error::new("RESOURCE_LIMIT", "Too many attachments."));
        }
        let mut unique = std::collections::BTreeSet::new();
        for id in &attachments {
            crate::domain::check_id(id)?;
            if !unique.insert(id.clone()) {
                return Err(Error::invalid("Duplicate attachment reference."));
            }
        }
        let attachment_json = serde_json::to_string(&attachments).map_err(|_| Error::storage())?;
        let recipient = s(&v, "recipient_assistant_id")?;
        let conversation = s(&v, "conversation_id")?;
        let idem = s(&v, "idempotency_key")?;
        let body = s(&v, "body")?;
        crate::domain::check_id(recipient)?;
        crate::domain::check_id(conversation)?;
        crate::domain::check_request_id(idem)?;
        if body.len() > self.limits.max_json_bytes {
            return Err(Error::invalid("Message too large."));
        }
        if self.encryption.is_none() {
            return Err(Error::new(
                "ENCRYPTION_REQUIRED",
                "Message encryption is not configured.",
            ));
        }
        let target = v.get("session_id").and_then(Value::as_str);
        if let Some(t) = target {
            crate::domain::check_id(t)?;
        }
        let msg = random_id("msg")?;
        let delivery = random_id("dlv")?;
        let fp = fingerprint(&json!([recipient, conversation, target, body, attachments]))?;
        let cipher = self.seal_field(&p.account_id, &format!("works-message:{msg}"), body)?;
        let now = self.clock.now_ms();
        let mut a = vec![
            msg.clone().into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            conversation.into(),
            idem.into(),
            fp.clone().into(),
            cipher.into(),
            now.into(),
            p.account_id.clone().into(),
            recipient.into(),
            target.map(str::to_owned).into(),
            p.account_id.clone().into(),
            recipient.into(),
            target.map(str::to_owned).into(),
            attachment_json.clone().into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            now.into(),
        ];
        a.extend(ga.clone());
        let message = Statement::new(
            format!(
                "INSERT INTO works_messages(message_id,account_id,sender_assistant_id,conversation_id,idempotency_key,fingerprint,ciphertext,created_at) SELECT ?,?,?,?,?,?,?,? WHERE EXISTS(SELECT 1 FROM works_assistants WHERE account_id=? AND assistant_id=? AND status='active' AND messages_enabled=1) AND (? IS NULL OR EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=? AND s.assistant_id=? AND s.session_id=? AND s.status='registered' AND i.status='active' AND t.status='active')) AND NOT EXISTS(SELECT 1 FROM json_each(?) j WHERE NOT EXISTS(SELECT 1 FROM works_message_attachments b WHERE b.attachment_id=j.value AND b.account_id=? AND b.sender_assistant_id=? AND b.state='uploaded' AND b.expires_at>?)) AND {g} ON CONFLICT(account_id,sender_assistant_id,idempotency_key) DO NOTHING"
            ),
            a,
        );
        // Capture routing from the authoritative rows in the same D1 transaction.
        let mut a = vec![
            delivery.clone().into(),
            p.account_id.clone().into(),
            msg.clone().into(),
            recipient.into(),
            conversation.into(),
            (if target.is_some() {
                "session"
            } else {
                "assistant"
            })
            .into(),
            target.map(str::to_owned).into(),
            target.map(str::to_owned).into(),
            now.into(),
            now.into(),
            p.account_id.clone().into(),
            recipient.into(),
            msg.clone().into(),
            target.map(str::to_owned).into(),
            target.map(str::to_owned).into(),
        ];
        a.extend(ga.clone());
        let insert = Statement::new(
            format!(
                "INSERT INTO works_deliveries(delivery_id,account_id,message_id,assistant_id,conversation_id,target_mode,requested_session_id,assigned_session_id,state,created_at,updated_at) SELECT ?,?,?,?,?,?,?,COALESCE(?,c.owner_session_id,a.default_session_id),CASE WHEN c.active_delivery_id IS NOT NULL AND ? IS NOT NULL AND ?<>c.owner_session_id THEN 'needs_handoff' WHEN c.owner_session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.session_id=c.owner_session_id AND s.status='registered' AND i.status='active' AND t.status='active') THEN 'needs_handoff' WHEN COALESCE(?,c.owner_session_id,a.default_session_id) IS NULL THEN 'waiting_for_session' ELSE 'assigned' END,?,? FROM works_assistants a LEFT JOIN works_conversations c ON c.account_id=a.account_id AND c.assistant_id=a.assistant_id AND c.conversation_id=? WHERE a.account_id=? AND a.assistant_id=? AND EXISTS(SELECT 1 FROM works_messages WHERE message_id=?) AND (? IS NULL OR EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=a.account_id AND s.assistant_id=a.assistant_id AND s.session_id=? AND s.status='registered' AND i.status='active' AND t.status='active')) AND {g}"
            ),
            {
                let mut x = vec![
                    delivery.clone().into(),
                    p.account_id.clone().into(),
                    msg.clone().into(),
                    recipient.into(),
                    conversation.into(),
                    (if target.is_some() {
                        "session"
                    } else {
                        "assistant"
                    })
                    .into(),
                    target.map(str::to_owned).into(),
                    target.map(str::to_owned).into(),
                    target.map(str::to_owned).into(),
                    target.map(str::to_owned).into(),
                    target.map(str::to_owned).into(),
                    now.into(),
                    now.into(),
                    conversation.into(),
                    p.account_id.clone().into(),
                    recipient.into(),
                    msg.clone().into(),
                    target.map(str::to_owned).into(),
                    target.map(str::to_owned).into(),
                ];
                x.extend(ga.clone());
                x
            },
        );
        let _ = a;
        let owner = Statement::new(
            "INSERT INTO works_conversations(account_id,conversation_id,assistant_id,owner_session_id) SELECT account_id,conversation_id,assistant_id,assigned_session_id FROM works_deliveries WHERE message_id=? AND state='assigned' ON CONFLICT(account_id,conversation_id,assistant_id) DO UPDATE SET owner_session_id=COALESCE(works_conversations.owner_session_id,excluded.owner_session_id)",
            vec![msg.clone().into()],
        );
        let mut link = vec![
            msg.clone().into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            attachment_json.into(),
            now.into(),
            msg.clone().into(),
        ];
        link.extend(ga.clone());
        let link = Statement::new(
            format!(
                "UPDATE works_message_attachments SET state='linked',message_id=? WHERE account_id=? AND sender_assistant_id=? AND attachment_id IN (SELECT value FROM json_each(?)) AND state='uploaded' AND expires_at>? AND EXISTS(SELECT 1 FROM works_deliveries d WHERE d.message_id=?) AND {g}"
            ),
            link,
        );
        self.db.batch(vec![message, insert, owner, link]).await?;
        let rows=self.db.query(Statement::new("SELECT m.message_id,m.fingerprint,d.delivery_id,d.state,d.assigned_session_id FROM works_messages m LEFT JOIN works_deliveries d ON d.message_id=m.message_id WHERE m.account_id=? AND m.sender_assistant_id=? AND m.idempotency_key=?",vec![p.account_id.clone().into(),p.assistant_id.clone().into(),idem.into()])).await?;
        let r = rows
            .first()
            .ok_or_else(|| Error::new("REVISION_CONFLICT", "Message rejected."))?;
        if text(r, "fingerprint")? != fp {
            return Err(Error::new(
                "IDEMPOTENCY_CONFLICT",
                "Idempotency key already used.",
            ));
        }
        let Some(d) = optional_text(r, "delivery_id") else {
            return Err(Error::new("INVALID_TARGET", "Session is unavailable."));
        };
        Ok(
            json!({"message_id":text(r,"message_id")?,"delivery_id":d,"state":text(r,"state")?,"assigned_session_id":optional_text(r,"assigned_session_id")}),
        )
    }
    async fn works_read(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        let session = s(&v, "session_id")?;
        // An expired lease is evidence of an unknown outcome, never permission to rerun.
        let (g, ga) = self.works_v2_guard(p);
        let now = self.clock.now_ms();
        let mut args = vec![
            now.into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            session.into(),
            now.into(),
            p.installation_id.clone().into(),
        ];
        args.extend(ga);
        self.db.batch(vec![Statement::new(format!("UPDATE works_deliveries SET state='unknown',updated_at=? WHERE account_id=? AND assistant_id=? AND assigned_session_id=? AND state='claimed' AND lease_expires_at<=? AND EXISTS(SELECT 1 FROM works_sessions s WHERE s.account_id=works_deliveries.account_id AND s.assistant_id=works_deliveries.assistant_id AND s.session_id=works_deliveries.assigned_session_id AND s.installation_id=?) AND {g}"),args)]).await?;
        let (g, ga) = self.works_v2_guard(p);
        let mut args = vec![
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            session.into(),
        ];
        args.extend(ga);
        let rows=self.db.query(Statement::new(format!("SELECT d.delivery_id,d.message_id,d.conversation_id,d.assigned_session_id,d.assignment_generation,d.state,d.claim_generation,d.conversation_generation,d.lease_expires_at,m.sender_assistant_id,m.ciphertext FROM works_deliveries d JOIN works_messages m ON m.message_id=d.message_id JOIN works_sessions s ON s.session_id=d.assigned_session_id AND s.account_id=d.account_id AND s.assistant_id=d.assistant_id WHERE d.account_id=? AND d.assistant_id=? AND d.assigned_session_id=? AND d.state IN ('assigned','claimed','unknown','needs_handoff') AND {g} ORDER BY d.created_at,d.delivery_id LIMIT 100"),args)).await?;
        self.works_check_current(p).await?;
        let mut result = Vec::new();
        for r in rows {
            let id = text(&r, "message_id")?;
            let body = self.open_field(
                &p.account_id,
                &format!("works-message:{id}"),
                &text(&r, "ciphertext")?,
            )?;
            let mut item = rows_json(
                vec![r],
                &[
                    "delivery_id",
                    "message_id",
                    "conversation_id",
                    "assigned_session_id",
                    "assignment_generation",
                    "state",
                    "claim_generation",
                    "conversation_generation",
                    "lease_expires_at",
                    "sender_assistant_id",
                ],
            );
            let (g, ga) = self.works_v2_guard(p);
            let mut args = vec![p.account_id.clone().into(), id.clone().into()];
            args.extend(ga);
            let refs=self.db.query(Statement::new(format!("SELECT attachment_id FROM works_message_attachments WHERE account_id=? AND message_id=? AND state='linked' AND {g} ORDER BY attachment_id"),args)).await?;
            item[0]["attachments"] = json!(
                refs.iter()
                    .map(|r| text(r, "attachment_id"))
                    .collect::<Result<Vec<_>>>()?
            );
            item[0]["body"] = json!(body);
            result.push(item[0].clone());
        }
        Ok(json!({"deliveries":result}))
    }
    async fn works_claim(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        let id = s(&v, "delivery_id")?;
        let session = s(&v, "session_id")?;
        let attempt = s(&v, "attempt_id")?;
        crate::domain::check_request_id(attempt)?;
        let generation = number(&v, "assignment_generation")?;
        let now = self.clock.now_ms();
        let lease = v.get("lease_seconds").and_then(Value::as_i64).unwrap_or(60);
        if !(1..=300).contains(&lease) {
            return Err(Error::invalid("Lease must be between 1 and 300 seconds."));
        }
        let (g, ga) = self.works_v2_guard(p);
        let mut seed = vec![
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            id.into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            session.into(),
            p.installation_id.clone().into(),
            generation.into(),
        ];
        seed.extend(ga.clone());
        let mut lock = vec![
            id.into(),
            session.into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            id.into(),
            session.into(),
            p.installation_id.clone().into(),
            generation.into(),
        ];
        lock.extend(ga.clone());
        let mut acquire = vec![
            attempt.into(),
            (now + lease * 1000).into(),
            now.into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            id.into(),
            session.into(),
            generation.into(),
        ];
        acquire.extend(ga);
        self.db.batch(vec![
            Statement::new(format!("INSERT INTO works_conversations(account_id,conversation_id,assistant_id) SELECT d.account_id,d.conversation_id,d.assistant_id FROM works_deliveries d JOIN works_sessions s ON s.account_id=d.account_id AND s.assistant_id=d.assistant_id AND s.session_id=d.assigned_session_id WHERE d.account_id=? AND d.assistant_id=? AND d.delivery_id=? AND d.account_id=? AND d.assistant_id=? AND s.session_id=? AND s.installation_id=? AND d.assignment_generation=? AND d.state='assigned' AND s.status='registered' AND {g} ON CONFLICT DO NOTHING"),seed),
            Statement::new(format!("UPDATE works_conversations SET active_delivery_id=?,generation=generation+1,owner_session_id=COALESCE(owner_session_id,?) WHERE account_id=? AND assistant_id=? AND active_delivery_id IS NULL AND EXISTS(SELECT 1 FROM works_deliveries d JOIN works_sessions s ON s.session_id=d.assigned_session_id AND s.account_id=d.account_id AND s.assistant_id=d.assistant_id WHERE d.delivery_id=? AND d.account_id=works_conversations.account_id AND d.assistant_id=works_conversations.assistant_id AND d.conversation_id=works_conversations.conversation_id AND s.session_id=? AND s.installation_id=? AND d.assignment_generation=? AND d.state='assigned' AND s.status='registered') AND {g}"),lock),
            Statement::new(format!("UPDATE works_deliveries SET state='claimed',claim_attempt_id=?,claim_generation=claim_generation+1,conversation_generation=(SELECT generation FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id),lease_expires_at=?,updated_at=? WHERE account_id=? AND assistant_id=? AND delivery_id=? AND assigned_session_id=? AND assignment_generation=? AND state='assigned' AND EXISTS(SELECT 1 FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id AND c.active_delivery_id=works_deliveries.delivery_id) AND {g}"),acquire)
        ]).await?;
        let rows=self.db.query(Statement::new("SELECT claim_generation,conversation_generation,lease_expires_at FROM works_deliveries WHERE account_id=? AND assistant_id=? AND delivery_id=? AND assigned_session_id=? AND assignment_generation=? AND state='claimed' AND claim_attempt_id=? AND lease_expires_at>?",vec![p.account_id.clone().into(),p.assistant_id.clone().into(),id.into(),session.into(),generation.into(),attempt.into(),now.into()])).await?;
        let r = rows.first().ok_or_else(|| {
            Error::new(
                "CLAIM_CONFLICT",
                "Delivery or conversation is not claimable.",
            )
        })?;
        Ok(
            json!({"delivery_id":id,"assignment_generation":generation,"claim_generation":integer(r,"claim_generation")?,"conversation_generation":integer(r,"conversation_generation")?,"lease_expires_at":integer(r,"lease_expires_at")?}),
        )
    }
    async fn works_finish(&self, p: &WorksPrincipal, v: Value, renew: bool) -> Result<Value> {
        let id = s(&v, "delivery_id")?;
        let session = s(&v, "session_id")?;
        let assignment = number(&v, "assignment_generation")?;
        let claim = number(&v, "claim_generation")?;
        let conversation = number(&v, "conversation_generation")?;
        let now = self.clock.now_ms();
        let outcome = if renew { "claimed" } else { s(&v, "outcome")? };
        if !["claimed", "completed", "failed", "unknown"].contains(&outcome)
            || (!renew && outcome == "claimed")
        {
            return Err(Error::invalid("Invalid outcome."));
        }
        let lease = v.get("lease_seconds").and_then(Value::as_i64).unwrap_or(60);
        if renew && !(1..=300).contains(&lease) {
            return Err(Error::invalid("Invalid lease."));
        }
        let (g, ga) = self.works_v2_guard(p);
        let mut a = vec![
            outcome.into(),
            if renew {
                Sql::Integer(now + lease * 1000)
            } else {
                Sql::Null
            },
            now.into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            id.into(),
            session.into(),
            assignment.into(),
            claim.into(),
            conversation.into(),
            now.into(),
            p.installation_id.clone().into(),
        ];
        a.extend(ga.clone());
        let mut statements = vec![Statement::new(
            format!(
                "UPDATE works_deliveries SET state=?,lease_expires_at=?,updated_at=? WHERE account_id=? AND assistant_id=? AND delivery_id=? AND assigned_session_id=? AND assignment_generation=? AND claim_generation=? AND conversation_generation=? AND state='claimed' AND lease_expires_at>? AND EXISTS(SELECT 1 FROM works_sessions s WHERE s.account_id=works_deliveries.account_id AND s.assistant_id=works_deliveries.assistant_id AND s.session_id=works_deliveries.assigned_session_id AND s.installation_id=? AND s.status='registered') AND EXISTS(SELECT 1 FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id AND c.active_delivery_id=works_deliveries.delivery_id AND c.generation=works_deliveries.conversation_generation) AND {g}"
            ),
            a,
        )];
        if !renew && outcome != "unknown" {
            let mut a = vec![
                p.account_id.clone().into(),
                p.assistant_id.clone().into(),
                id.into(),
                conversation.into(),
                outcome.into(),
                assignment.into(),
                claim.into(),
            ];
            a.extend(ga);
            statements.push(Statement::new(format!("UPDATE works_conversations SET active_delivery_id=NULL WHERE account_id=? AND assistant_id=? AND active_delivery_id=? AND generation=? AND EXISTS(SELECT 1 FROM works_deliveries d WHERE d.delivery_id=works_conversations.active_delivery_id AND d.state=? AND d.assignment_generation=? AND d.claim_generation=?) AND {g}"),a));
        }
        self.db.batch(statements).await?;
        let rows=self.db.query(Statement::new("SELECT state,lease_expires_at,updated_at FROM works_deliveries WHERE account_id=? AND assistant_id=? AND delivery_id=? AND assigned_session_id=? AND assignment_generation=? AND claim_generation=? AND conversation_generation=?",vec![p.account_id.clone().into(),p.assistant_id.clone().into(),id.into(),session.into(),assignment.into(),claim.into(),conversation.into()])).await?;
        let r = rows
            .first()
            .ok_or_else(|| Error::new("CLAIM_LOST", "Claim is no longer valid."))?;
        if text(r, "state")? != outcome || integer(r, "updated_at")? != now {
            return Err(Error::new("CLAIM_LOST", "Claim is no longer valid."));
        }
        Ok(
            json!({"delivery_id":id,"state":outcome,"lease_expires_at":if renew {Some(now+lease*1000)} else {None}}),
        )
    }
    /// Cookie-authenticated owner control plane; execution claims remain client-only.
    pub(super) async fn works_account_dispatch(
        &self,
        r: &Request,
        operation: &str,
        mut v: Value,
    ) -> Result<Value> {
        let session = self.account_session(r, true).await?;
        let account = text(&session, "account_id")?;
        let now = self.clock.now_ms();
        let digest =
            Sha256::digest(accounts::account_cookie(r, &self.config.public_origin)?.as_bytes())
                .to_vec();
        let (identity, identity_args) = self.unified_identity_guard("a");
        let g = format!(
            "EXISTS(SELECT 1 FROM account_sessions s JOIN accounts a ON a.account_id=s.account_id WHERE s.session_digest=? AND s.account_id=? AND s.expires_at>? AND s.created_at>=? AND a.disabled=0 AND {identity})"
        );
        let mut ga = vec![
            digest.into(),
            account.clone().into(),
            now.into(),
            (now - 900000).into(),
        ];
        ga.extend(identity_args);
        let assistant = v
            .get("assistant_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match operation {
            "sessions_list" => {
                let mut args = vec![now.into(), account.clone().into(), assistant.clone().into()];
                args.extend(ga.clone());
                let rows=self.db.query(Statement::new(format!("SELECT s.session_id,s.display_name,s.host_kind,s.status,s.capability,s.discoverable,s.last_seen_at,EXISTS(SELECT 1 FROM works_installations ai JOIN works_attachments at ON at.account_id=ai.account_id AND at.installation_id=ai.installation_id WHERE ai.account_id=s.account_id AND ai.installation_id=s.installation_id AND ai.status='active' AND at.assistant_id=s.assistant_id AND at.status='active') AS eligible,(SELECT x.session_id FROM works_sessions x JOIN works_installations i ON i.account_id=x.account_id AND i.installation_id=x.installation_id JOIN works_attachments t ON t.account_id=x.account_id AND t.assistant_id=x.assistant_id AND t.installation_id=x.installation_id WHERE x.account_id=s.account_id AND x.assistant_id=s.assistant_id AND x.session_id<>s.session_id AND x.status='registered' AND i.status='active' AND t.status='active' ORDER BY CASE WHEN x.capability='wakeable' AND x.last_seen_at>=?-120000 THEN 0 ELSE 1 END,x.created_at,x.session_id LIMIT 1) AS replacement_session_id FROM works_sessions s WHERE s.account_id=? AND s.assistant_id=? AND s.status<>'deleted' AND {g} ORDER BY s.created_at,s.session_id"),args)).await?;
                self.account_session(r, true).await?;
                Ok(
                    json!({"sessions":rows_json(rows,&["session_id","display_name","host_kind","status","capability","discoverable","last_seen_at","eligible","replacement_session_id"])}),
                )
            }
            "recipients_list" => {
                let mut args = vec![account.clone().into()];
                args.extend(ga.clone());
                let a=self.db.query(Statement::new(format!("SELECT a.assistant_id,a.name,a.default_session_id,(SELECT s.display_name FROM works_sessions s WHERE s.account_id=a.account_id AND s.assistant_id=a.assistant_id AND s.session_id=a.default_session_id) AS default_session_name FROM works_assistants a WHERE a.account_id=? AND a.status='active' AND a.messages_enabled=1 AND {g} ORDER BY a.name,a.assistant_id"),args)).await?;
                let mut recipients = Vec::new();
                for x in a {
                    let id = text(&x, "assistant_id")?;
                    let mut args = vec![account.clone().into(), id.clone().into()];
                    args.extend(ga.clone());
                    let rows=self.db.query(Statement::new(format!("SELECT s.session_id,s.display_name FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=? AND s.assistant_id=? AND s.discoverable=1 AND s.status='registered' AND i.status='active' AND t.status='active' AND {g} ORDER BY s.created_at,s.session_id"),args)).await?;
                    recipients.push(json!({"assistant_id":id,"name":text(&x,"name")?,"default_session_id":optional_text(&x,"default_session_id"),"default_session_name":optional_text(&x,"default_session_name"),"sessions":rows_json(rows,&["session_id","display_name"])}));
                }
                self.account_session(r, true).await?;
                Ok(json!({"recipients":recipients}))
            }
            "messages_read" => {
                let mut args = vec![
                    now.into(),
                    account.clone().into(),
                    assistant.clone().into(),
                    now.into(),
                ];
                args.extend(ga.clone());
                self.db.batch(vec![Statement::new(format!("UPDATE works_deliveries SET state='unknown',updated_at=? WHERE account_id=? AND assistant_id=? AND state='claimed' AND lease_expires_at<=? AND {g}"),args)]).await?;
                let mut args = vec![account.clone().into(), assistant.clone().into()];
                args.extend(ga.clone());
                let rows=self.db.query(Statement::new(format!("SELECT d.delivery_id,d.state,d.assignment_generation,COALESCE(c.generation,0) AS conversation_generation,d.assigned_session_id,m.message_id,m.sender_assistant_id,m.ciphertext FROM works_deliveries d JOIN works_messages m ON m.message_id=d.message_id LEFT JOIN works_conversations c ON c.account_id=d.account_id AND c.assistant_id=d.assistant_id AND c.conversation_id=d.conversation_id WHERE d.account_id=? AND d.assistant_id=? AND {g} ORDER BY d.created_at DESC LIMIT 100"),args)).await?;
                let mut out = Vec::new();
                for x in rows {
                    let id = text(&x, "message_id")?;
                    let content = self.open_field(
                        &account,
                        &format!("works-message:{id}"),
                        &text(&x, "ciphertext")?,
                    )?;
                    let mut args = vec![account.clone().into(), id.clone().into()];
                    args.extend(ga.clone());
                    let refs=self.db.query(Statement::new(format!("SELECT attachment_id FROM works_message_attachments WHERE account_id=? AND message_id=? AND state='linked' AND {g} ORDER BY attachment_id"),args)).await?;
                    let attachments = refs
                        .iter()
                        .map(|r| text(r, "attachment_id"))
                        .collect::<Result<Vec<_>>>()?;
                    out.push(json!({"delivery_id":text(&x,"delivery_id")?,"message_id":id,"sender_assistant_id":text(&x,"sender_assistant_id")?,"content":content,"status":text(&x,"state")?,"assignment_generation":integer(&x,"assignment_generation")?,"conversation_generation":integer(&x,"conversation_generation")?,"assigned_session_id":optional_text(&x,"assigned_session_id"),"attachments":attachments}));
                }
                self.account_session(r, true).await?;
                Ok(json!({"messages":out}))
            }
            "messages_send" => {
                let rows=self.db.query(Statement::new("SELECT policy_revision FROM works_assistants WHERE account_id=? AND assistant_id=? AND status='active' AND messages_enabled=1",vec![account.clone().into(),assistant.clone().into()])).await?;
                let rev = integer(
                    rows.first().ok_or_else(Error::not_found)?,
                    "policy_revision",
                )?;
                let p = WorksPrincipal {
                    account_id: account,
                    assistant_id: assistant,
                    installation_id: String::new(),
                    family_id: String::new(),
                    token_digest: Vec::new(),
                    policy_revision: rev,
                    messages_enabled: true,
                    keys_read: false,
                    keys_write: false,
                    keys_create: false,
                    key_scope: String::new(),
                };
                if v.get("conversation_id").is_none() {
                    v["conversation_id"] = json!(format!(
                        "conv_{}",
                        hex::encode(
                            &Sha256::digest(
                                format!("{}:{}", p.assistant_id, s(&v, "idempotency_key")?)
                                    .as_bytes()
                            )[..16]
                        )
                    ));
                }
                if let Some(content) = v.get("content").cloned() {
                    v["body"] = content;
                }
                let owner_guard = format!(
                    "({g}) AND EXISTS(SELECT 1 FROM works_assistants a WHERE a.account_id=? AND a.assistant_id=? AND a.status='active' AND a.messages_enabled=1 AND a.policy_revision=?)"
                );
                let mut owner_args = ga;
                owner_args.extend([
                    p.account_id.clone().into(),
                    p.assistant_id.clone().into(),
                    p.policy_revision.into(),
                ]);
                self.works_send_guard(&p, v, owner_guard, owner_args).await
            }
            "deliveries_handoff" => {
                if v.get("confirm_unknown").and_then(Value::as_bool) != Some(true) {
                    return Err(Error::invalid(
                        "Confirm that the previous execution was checked before handoff.",
                    ));
                }
                let id = s(&v, "delivery_id")?;
                let dest = s(&v, "session_id")?;
                let assignment = number(&v, "assignment_generation")?;
                let generation = number(&v, "conversation_generation")?;
                let mut seed = vec![
                    account.clone().into(),
                    assistant.clone().into(),
                    id.into(),
                    assignment.into(),
                ];
                seed.extend(ga.clone());
                let mut change = vec![
                    dest.into(),
                    account.clone().into(),
                    assistant.clone().into(),
                    generation.into(),
                    id.into(),
                    assignment.into(),
                    now.into(),
                    dest.into(),
                ];
                change.extend(ga.clone());
                let handoff_id = random_id("handoff")?;
                let mut move_delivery = vec![
                    dest.into(),
                    handoff_id.clone().into(),
                    now.into(),
                    account.clone().into(),
                    assistant.clone().into(),
                    id.into(),
                    assignment.into(),
                    generation.into(),
                    dest.into(),
                ];
                move_delivery.extend(ga.clone());
                let mut audit = vec![
                    handoff_id.clone().into(),
                    dest.into(),
                    generation.into(),
                    now.into(),
                    account.clone().into(),
                    assistant.clone().into(),
                    id.into(),
                    assignment.into(),
                    dest.into(),
                    handoff_id.into(),
                ];
                audit.extend(ga.clone());
                let result=self.db.batch(vec![
                    Statement::new(format!("INSERT INTO works_conversations(account_id,conversation_id,assistant_id) SELECT account_id,conversation_id,assistant_id FROM works_deliveries WHERE account_id=? AND assistant_id=? AND delivery_id=? AND assignment_generation=? AND {g} ON CONFLICT DO NOTHING"),seed),
                    Statement::new(format!("UPDATE works_conversations SET owner_session_id=?,active_delivery_id=NULL,generation=generation+1 WHERE account_id=? AND assistant_id=? AND generation=? AND EXISTS(SELECT 1 FROM works_deliveries d WHERE d.account_id=works_conversations.account_id AND d.assistant_id=works_conversations.assistant_id AND d.conversation_id=works_conversations.conversation_id AND d.delivery_id=? AND d.assignment_generation=? AND d.state IN ('needs_handoff','unknown','claimed') AND (d.state<>'claimed' OR d.lease_expires_at<=?) AND (works_conversations.active_delivery_id IS NULL OR works_conversations.active_delivery_id=d.delivery_id)) AND EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=works_conversations.account_id AND s.assistant_id=works_conversations.assistant_id AND s.session_id=? AND s.status='registered' AND i.status='active' AND t.status='active') AND {g}"),change),
                    Statement::new(format!("UPDATE works_deliveries SET assigned_session_id=?,claim_attempt_id=?,assignment_generation=assignment_generation+1,state='assigned',lease_expires_at=NULL,updated_at=? WHERE account_id=? AND assistant_id=? AND delivery_id=? AND assignment_generation=? AND state IN ('needs_handoff','unknown','claimed') AND EXISTS(SELECT 1 FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id AND c.generation=?+1 AND c.owner_session_id=? AND c.active_delivery_id IS NULL) AND {g} RETURNING assignment_generation"),move_delivery),
                    Statement::new(format!("INSERT INTO works_handoff_audit(event_id,account_id,assistant_id,delivery_id,destination_session_id,assignment_generation,conversation_generation,created_at) SELECT ?,account_id,assistant_id,delivery_id,?,assignment_generation,?,? FROM works_deliveries WHERE account_id=? AND assistant_id=? AND delivery_id=? AND assignment_generation=?+1 AND assigned_session_id=? AND claim_attempt_id=? AND state='assigned' AND {g}"),audit)
                ]).await?;
                let row = result.get(2).and_then(|r| r.first()).ok_or_else(|| {
                    Error::new(
                        "REVISION_CONFLICT",
                        "Delivery or conversation changed; reload before handoff.",
                    )
                })?;
                Ok(
                    json!({"delivery_id":id,"assignment_generation":integer(row,"assignment_generation")?}),
                )
            }
            "sessions_update" => {
                if v.get("status").and_then(Value::as_str) == Some("deleted")
                    && v.get("default_session_id").is_some()
                {
                    let id = s(&v, "session_id")?;
                    let target = v.get("default_session_id").unwrap();
                    if !(target.is_null() || target.is_string()) {
                        return Err(Error::invalid("Invalid default session."));
                    }
                    let dest = target.as_str().map(str::to_owned);
                    if dest.as_deref() == Some(id) {
                        return Err(Error::invalid("Removed session cannot remain the default."));
                    }
                    let token = random_id("mutation")?;
                    let mut remove = vec![
                        token.clone().into(),
                        account.clone().into(),
                        assistant.clone().into(),
                        id.into(),
                        dest.clone().into(),
                        dest.clone().into(),
                    ];
                    remove.extend(ga.clone());
                    let mut handoff = vec![
                        now.into(),
                        account.clone().into(),
                        assistant.clone().into(),
                        id.into(),
                        token.clone().into(),
                    ];
                    handoff.extend(ga.clone());
                    let mut set = vec![
                        dest.clone().into(),
                        account.clone().into(),
                        assistant.clone().into(),
                        id.into(),
                        token.into(),
                    ];
                    set.extend(ga.clone());
                    let result=self.db.batch(vec![
                        Statement::new(format!("UPDATE works_sessions SET status='deleted',route_revision=route_revision+1,mutation_token=? WHERE account_id=? AND assistant_id=? AND session_id=? AND status<>'deleted' AND (? IS NULL OR EXISTS(SELECT 1 FROM works_sessions x JOIN works_installations i ON i.account_id=x.account_id AND i.installation_id=x.installation_id JOIN works_attachments t ON t.account_id=x.account_id AND t.assistant_id=x.assistant_id AND t.installation_id=x.installation_id WHERE x.account_id=works_sessions.account_id AND x.assistant_id=works_sessions.assistant_id AND x.session_id=? AND x.status='registered' AND i.status='active' AND t.status='active')) AND {g} RETURNING session_id"),remove),
                        Statement::new(format!("UPDATE works_deliveries SET state='needs_handoff',assignment_generation=assignment_generation+1,updated_at=? WHERE account_id=? AND assistant_id=? AND assigned_session_id=? AND state IN ('assigned','claimed','unknown') AND EXISTS(SELECT 1 FROM works_sessions s WHERE s.session_id=works_deliveries.assigned_session_id AND s.mutation_token=?) AND {g}"),handoff),
                        Statement::new(format!("UPDATE works_assistants SET default_session_id=?,route_revision=route_revision+1 WHERE account_id=? AND assistant_id=? AND default_session_id=? AND EXISTS(SELECT 1 FROM works_sessions s WHERE s.session_id=works_assistants.default_session_id AND s.mutation_token=?) AND {g}"),set)
                    ]).await?;
                    if result[0].is_empty() {
                        return Err(Error::new(
                            "REVISION_CONFLICT",
                            "Session or replacement is unavailable.",
                        ));
                    }
                    return Ok(json!({"ok":true}));
                }
                if v.get("default_session_id").is_some()
                    && (v.get("status").is_some() || v.get("discoverable").is_some())
                {
                    return Err(Error::invalid("Set the default in a separate update."));
                }
                if v.get("status").is_some() && v.get("discoverable").is_some() {
                    return Err(Error::invalid(
                        "Change session status and visibility separately.",
                    ));
                }
                let mut updates = Vec::new();
                let mut required = Vec::new();
                if let Some(id) = v.get("session_id").and_then(Value::as_str) {
                    if let Some(status) = v.get("status").and_then(Value::as_str) {
                        if !["registered", "paused", "deleted"].contains(&status) {
                            return Err(Error::invalid("Invalid status."));
                        }
                        let mut a = vec![
                            status.into(),
                            account.clone().into(),
                            assistant.clone().into(),
                            id.into(),
                        ];
                        a.extend(ga.clone());
                        required.push(updates.len());
                        updates.push(Statement::new(format!("UPDATE works_sessions SET status=?,route_revision=route_revision+1 WHERE account_id=? AND assistant_id=? AND session_id=? AND status<>'deleted' AND {g} RETURNING session_id"),a));
                        if status == "deleted" {
                            let mut a = vec![
                                now.into(),
                                account.clone().into(),
                                assistant.clone().into(),
                                id.into(),
                            ];
                            a.extend(ga.clone());
                            updates.push(Statement::new(format!("UPDATE works_deliveries SET state='needs_handoff',assignment_generation=assignment_generation+1,updated_at=? WHERE account_id=? AND assistant_id=? AND assigned_session_id=? AND state IN ('assigned','claimed','unknown') AND {g}"),a));
                            let mut a = vec![
                                now.into(),
                                account.clone().into(),
                                assistant.clone().into(),
                                id.into(),
                            ];
                            a.extend(ga.clone());
                            updates.push(Statement::new(format!("UPDATE works_assistants SET default_session_id=(SELECT s.session_id FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=works_assistants.account_id AND s.assistant_id=works_assistants.assistant_id AND s.status='registered' AND i.status='active' AND t.status='active' ORDER BY CASE WHEN s.capability='wakeable' AND s.last_seen_at>=?-120000 THEN 0 ELSE 1 END,s.created_at,s.session_id LIMIT 1),route_revision=route_revision+1 WHERE account_id=? AND assistant_id=? AND default_session_id=? AND {g}"),a));
                        }
                    }
                    if let Some(discoverable) = v.get("discoverable").and_then(Value::as_bool) {
                        let mut a = vec![
                            discoverable.into(),
                            account.clone().into(),
                            assistant.clone().into(),
                            id.into(),
                        ];
                        a.extend(ga.clone());
                        required.push(updates.len());
                        updates.push(Statement::new(format!("UPDATE works_sessions SET discoverable=? WHERE account_id=? AND assistant_id=? AND session_id=? AND status<>'deleted' AND {g} RETURNING session_id"),a));
                    }
                }
                if let Some(def) = v.get("default_session_id") {
                    let dest = def.as_str().map(str::to_owned);
                    let mut a = vec![
                        dest.clone().into(),
                        account.clone().into(),
                        assistant.clone().into(),
                        dest.clone().into(),
                        dest.clone().into(),
                    ];
                    a.extend(ga.clone());
                    required.push(updates.len());
                    updates.push(Statement::new(format!("UPDATE works_assistants SET default_session_id=?,route_revision=route_revision+1 WHERE account_id=? AND assistant_id=? AND (? IS NULL OR EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.account_id=works_assistants.account_id AND s.assistant_id=works_assistants.assistant_id AND s.session_id=? AND s.status='registered' AND i.status='active' AND t.status='active')) AND {g} RETURNING assistant_id"),a));
                    if let Some(id) = dest {
                        let mut a = vec![
                            id.clone().into(),
                            now.into(),
                            account.clone().into(),
                            assistant.clone().into(),
                            id.into(),
                        ];
                        a.extend(ga.clone());
                        updates.push(Statement::new(format!("UPDATE works_deliveries SET assigned_session_id=COALESCE((SELECT c.owner_session_id FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id),?),state=CASE WHEN EXISTS(SELECT 1 FROM works_conversations c WHERE c.account_id=works_deliveries.account_id AND c.assistant_id=works_deliveries.assistant_id AND c.conversation_id=works_deliveries.conversation_id AND c.owner_session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM works_sessions s JOIN works_installations i ON i.account_id=s.account_id AND i.installation_id=s.installation_id JOIN works_attachments t ON t.account_id=s.account_id AND t.assistant_id=s.assistant_id AND t.installation_id=s.installation_id WHERE s.session_id=c.owner_session_id AND s.status='registered' AND i.status='active' AND t.status='active')) THEN 'needs_handoff' ELSE 'assigned' END,assignment_generation=assignment_generation+1,updated_at=? WHERE account_id=? AND assistant_id=? AND state='waiting_for_session' AND target_mode='assistant' AND EXISTS(SELECT 1 FROM works_assistants a WHERE a.account_id=works_deliveries.account_id AND a.assistant_id=works_deliveries.assistant_id AND a.default_session_id=?) AND {g}"),a));
                    }
                }
                if updates.is_empty() {
                    return Err(Error::invalid("No update supplied."));
                }
                let result = self.db.batch(updates).await?;
                if required.iter().any(|i| result[*i].is_empty()) {
                    return Err(Error::new(
                        "REVISION_CONFLICT",
                        "Session or default target is unavailable.",
                    ));
                }
                Ok(json!({"ok":true}))
            }
            _ => Err(Error::not_found()),
        }
    }
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use crate::native::{FileStore, SqliteDb};
    #[tokio::test]
    async fn routing_claim_fencing_and_encrypted_body() {
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
            Statement::new("INSERT INTO accounts(account_id,issuer,subject,created_at) VALUES ('a','i','s',0)",vec![]),
            Statement::new("INSERT INTO works_installations(installation_id,account_id,label,created_at) VALUES ('inst','a','device',0)",vec![]),
            Statement::new("INSERT INTO works_families(family_id,account_id,client_id,resource,installation_id,created_at,expires_at) VALUES ('family','a','client','http://localhost/mcp','inst',0,9999999999999)",vec![]),
            Statement::new("UPDATE works_families SET client_revision=1 WHERE family_id='family'",vec![]),
            Statement::new("INSERT INTO works_access_tokens(token_digest,family_id,expires_at) VALUES (?,?,?)",vec![vec![7u8;32].into(),"family".into(),9999999999999i64.into()]),
            Statement::new("INSERT INTO works_assistants(assistant_id,account_id,name,messages_enabled,created_at) VALUES ('sender','a','sender',1,0),('receiver','a','receiver',1,0)",vec![]),
            Statement::new("INSERT INTO works_attachments(attachment_id,account_id,assistant_id,installation_id,status,created_at) VALUES ('att1','a','sender','inst','active',0),('att2','a','receiver','inst','active',0)",vec![]),
        ]).await.unwrap();
        let p = |id: &str| WorksPrincipal {
            account_id: "a".into(),
            assistant_id: id.into(),
            installation_id: "inst".into(),
            family_id: "family".into(),
            token_digest: vec![7u8; 32],
            policy_revision: 1,
            messages_enabled: true,
            keys_read: false,
            keys_write: false,
            keys_create: false,
            key_scope: "all".into(),
        };
        let sender = p("sender");
        let receiver = p("receiver");
        app.db.batch(vec![
            Statement::new("INSERT INTO works_installations(installation_id,account_id,label,created_at) VALUES ('inst2','a','device2',0)",vec![]),
            Statement::new("INSERT INTO works_families(family_id,account_id,client_id,client_revision,resource,installation_id,created_at,expires_at) VALUES ('family2','a','client',1,'http://localhost/mcp','inst2',0,9999999999999)",vec![]),
            Statement::new("INSERT INTO works_access_tokens(token_digest,family_id,expires_at) VALUES (?,?,?)",vec![vec![8u8;32].into(),"family2".into(),9999999999999i64.into()]),
            Statement::new("INSERT INTO works_attachments(attachment_id,account_id,assistant_id,installation_id,status,created_at) VALUES ('att3','a','receiver','inst2','active',0)",vec![])
        ]).await.unwrap();
        let mut receiver2 = p("receiver");
        receiver2.installation_id = "inst2".into();
        receiver2.family_id = "family2".into();
        receiver2.token_digest = vec![8u8; 32];
        let reg = app
            .works_sessions_dispatch(
                &receiver,
                "sessions_register",
                json!({"host_kind":"codex","external_session_id":"thread","display_name":"Work"}),
            )
            .await
            .unwrap();
        let session = reg["session_id"].as_str().unwrap();
        let send = json!({"recipient_assistant_id":"receiver","conversation_id":"conv","idempotency_key":"req1","body":"private body"});
        let msg = app
            .works_sessions_dispatch(&sender, "messages_send", send.clone())
            .await
            .unwrap();
        assert_eq!(msg["state"], "waiting_for_session");
        let rows = app
            .db
            .query(Statement::new(
                "SELECT ciphertext FROM works_messages",
                vec![],
            ))
            .await
            .unwrap();
        assert!(
            !text(&rows[0], "ciphertext")
                .unwrap()
                .contains("private body")
        );
        assert_eq!(
            app.works_sessions_dispatch(&sender, "messages_send", send)
                .await
                .unwrap()["message_id"],
            msg["message_id"]
        );
        app.works_sessions_dispatch(
            &receiver,
            "sessions_update",
            json!({"session_id":session,"set_default":true}),
        )
        .await
        .unwrap();
        let read = app
            .works_sessions_dispatch(&receiver, "messages_read", json!({"session_id":session}))
            .await
            .unwrap();
        assert_eq!(read["deliveries"][0]["body"], "private body");
        let d = &read["deliveries"][0];
        let delivery = d["delivery_id"].as_str().unwrap();
        let assignment = d["assignment_generation"].as_i64().unwrap();
        let cross = app
            .works_sessions_dispatch(&receiver2, "messages_read", json!({"session_id":session}))
            .await
            .unwrap();
        assert_eq!(cross["deliveries"][0]["body"], "private body");
        assert!(app.works_sessions_dispatch(&receiver2,"deliveries_claim",json!({"session_id":session,"delivery_id":delivery,"assignment_generation":assignment,"attempt_id":"cross"})).await.is_err());
        let claimed=app.works_sessions_dispatch(&receiver,"deliveries_claim",json!({"session_id":session,"delivery_id":delivery,"assignment_generation":assignment,"attempt_id":"attempt1"})).await.unwrap();
        assert!(app.works_sessions_dispatch(&receiver,"deliveries_claim",json!({"session_id":session,"delivery_id":delivery,"assignment_generation":assignment,"attempt_id":"attempt2"})).await.is_err());
        let other = app
            .works_sessions_dispatch(
                &receiver2,
                "sessions_register",
                json!({"host_kind":"codex","external_session_id":"other","display_name":"Other"}),
            )
            .await
            .unwrap();
        let redirect=app.works_sessions_dispatch(&sender,"messages_send",json!({"recipient_assistant_id":"receiver","session_id":other["session_id"],"conversation_id":"conv","idempotency_key":"redirect","body":"redirect"})).await.unwrap();
        assert_eq!(redirect["state"], "needs_handoff");
        assert_eq!(redirect["assigned_session_id"], other["session_id"]);
        let mut finish = json!({"session_id":session,"delivery_id":delivery,"assignment_generation":assignment,"claim_generation":claimed["claim_generation"],"conversation_generation":claimed["conversation_generation"],"outcome":"completed"});
        let mut stale = finish.clone();
        stale["claim_generation"] = json!(999);
        assert!(
            app.works_sessions_dispatch(&receiver, "deliveries_complete", stale)
                .await
                .is_err()
        );
        app.works_sessions_dispatch(&receiver, "deliveries_complete", finish.take())
            .await
            .unwrap();
        let next=app.works_sessions_dispatch(&sender,"messages_send",json!({"recipient_assistant_id":"receiver","session_id":session,"conversation_id":"conv2","idempotency_key":"req2","body":"second"})).await.unwrap();
        let id = next["delivery_id"].as_str().unwrap();
        let claim=app.works_sessions_dispatch(&receiver,"deliveries_claim",json!({"session_id":session,"delivery_id":id,"assignment_generation":1,"attempt_id":"attempt3"})).await.unwrap();
        app.db
            .batch(vec![Statement::new(
                "UPDATE works_deliveries SET lease_expires_at=0 WHERE delivery_id=?",
                vec![id.into()],
            )])
            .await
            .unwrap();
        let read = app
            .works_sessions_dispatch(&receiver, "messages_read", json!({"session_id":session}))
            .await
            .unwrap();
        assert!(
            read["deliveries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["delivery_id"] == id && d["state"] == "unknown")
        );
        assert!(app.works_sessions_dispatch(&receiver,"deliveries_claim",json!({"session_id":session,"delivery_id":id,"assignment_generation":1,"attempt_id":"attempt4"})).await.is_err());
        assert!(app.works_sessions_dispatch(&receiver,"deliveries_complete",json!({"session_id":session,"delivery_id":id,"assignment_generation":1,"claim_generation":claim["claim_generation"],"conversation_generation":claim["conversation_generation"],"outcome":"completed"})).await.is_err());
        let data = b"confidential attachment";
        let hash = hex::encode(Sha256::digest(data));
        let reservation=app.works_attachments_dispatch(&sender,"attachments_prepare",json!({"request_id":"attachment_req","content_type":"text/plain","size_bytes":data.len(),"sha256":hash,"filename":"private.txt"})).await.unwrap();
        let attachment = reservation["attachment_id"].as_str().unwrap();
        assert!(app.works_attachments_dispatch(&sender,"attachments_upload",json!({"attachment_id":attachment,"content_base64":URL_SAFE_NO_PAD.encode(b"wrong")})).await.is_err());
        app.works_attachments_dispatch(
            &sender,
            "attachments_upload",
            json!({"attachment_id":attachment,"content_base64":URL_SAFE_NO_PAD.encode(data)}),
        )
        .await
        .unwrap();
        let stored = app
            .db
            .query(Statement::new(
                "SELECT object_key FROM works_message_attachments WHERE attachment_id=?",
                vec![attachment.into()],
            ))
            .await
            .unwrap();
        let raw_object = app
            .objects
            .read(&text(&stored[0], "object_key").unwrap())
            .await
            .unwrap();
        assert!(!raw_object.windows(data.len()).any(|w| w == data));
        assert!(app.works_sessions_dispatch(&receiver,"messages_send",json!({"recipient_assistant_id":"sender","conversation_id":"badattach","idempotency_key":"badattach","body":"no","attachments":[attachment]})).await.is_err());
        let with_attachment=app.works_sessions_dispatch(&sender,"messages_send",json!({"recipient_assistant_id":"receiver","session_id":session,"conversation_id":"attachconv","idempotency_key":"attachsend","body":"file","attachments":[attachment]})).await.unwrap();
        let download = app
            .works_attachments_dispatch(
                &receiver2,
                "attachments_read",
                json!({"message_id":with_attachment["message_id"],"attachment_id":attachment}),
            )
            .await
            .unwrap();
        assert_eq!(download["content_base64"], URL_SAFE_NO_PAD.encode(data));
        assert!(
            app.works_attachments_dispatch(
                &receiver,
                "attachments_read",
                json!({"message_id":"wrong","attachment_id":attachment})
            )
            .await
            .is_err()
        );
        let inbox = app
            .works_sessions_dispatch(&receiver, "messages_read", json!({"session_id":session}))
            .await
            .unwrap();
        assert!(
            inbox["deliveries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["message_id"] == with_attachment["message_id"]
                    && d["attachments"][0] == attachment)
        );
        app.db.batch(vec![Statement::new("INSERT INTO works_message_attachments(attachment_id,account_id,sender_assistant_id,request_id,fingerprint,object_key,content_type,size_bytes,sha256,state,created_at,expires_at) VALUES ('foreign','other','sender','foreign_req','f','object','text/plain',0,'00','uploaded',0,9999999999999)",vec![])]).await.unwrap();
        assert!(app.works_sessions_dispatch(&sender,"messages_send",json!({"recipient_assistant_id":"receiver","conversation_id":"foreignconv","idempotency_key":"foreignsend","body":"no","attachments":["foreign"]})).await.is_err());
        let expired=app.works_attachments_dispatch(&sender,"attachments_prepare",json!({"request_id":"expires","content_type":"text/plain","size_bytes":data.len(),"sha256":hash})).await.unwrap();
        let expired_id = expired["attachment_id"].as_str().unwrap();
        app.works_attachments_dispatch(
            &sender,
            "attachments_upload",
            json!({"attachment_id":expired_id,"content_base64":URL_SAFE_NO_PAD.encode(data)}),
        )
        .await
        .unwrap();
        let old = app
            .db
            .query(Statement::new(
                "SELECT object_key FROM works_message_attachments WHERE attachment_id=?",
                vec![expired_id.into()],
            ))
            .await
            .unwrap();
        let old_key = text(&old[0], "object_key").unwrap();
        app.db
            .batch(vec![Statement::new(
                "UPDATE works_message_attachments SET expires_at=0 WHERE attachment_id=?",
                vec![expired_id.into()],
            )])
            .await
            .unwrap();
        app.works_attachments_dispatch(&receiver,"attachments_prepare",json!({"request_id":"global_gc","content_type":"text/plain","size_bytes":data.len(),"sha256":hash})).await.unwrap();
        assert!(app.objects.inspect(&old_key).await.unwrap().is_none());
        app.works_attachments_dispatch(&sender,"attachments_prepare",json!({"request_id":"cleanup_next","content_type":"text/plain","size_bytes":data.len(),"sha256":hash})).await.unwrap();
        assert!(app.objects.inspect(&old_key).await.unwrap().is_none());
        let raw = "x".repeat(43);
        let now = app.clock.now_ms();
        app.db.batch(vec![Statement::new("INSERT INTO account_sessions(session_digest,account_id,created_at,expires_at) VALUES (?,'a',?,?)",vec![Sha256::digest(raw.as_bytes()).to_vec().into(),now.into(),(now+600000).into()])]).await.unwrap();
        let mut request = Request::json("POST", "/account/works/deliveries/handoff", "", json!({}));
        request
            .headers
            .insert("cookie".into(), format!("waypost_account={raw}"));
        request
            .headers
            .insert("origin".into(), "http://localhost".into());
        request
            .headers
            .insert("x-waypost-csrf".into(), app.csrf_token(&raw).unwrap());
        let handoff = json!({"assistant_id":"receiver","delivery_id":id,"session_id":other["session_id"],"assignment_generation":1,"conversation_generation":claim["conversation_generation"],"confirm_unknown":true});
        let mut stale_handoff = handoff.clone();
        stale_handoff["conversation_generation"] = json!(999);
        assert!(
            app.works_account_dispatch(&request, "deliveries_handoff", stale_handoff)
                .await
                .is_err()
        );
        let moved = app
            .works_account_dispatch(&request, "deliveries_handoff", handoff.clone())
            .await
            .unwrap();
        assert_eq!(moved["assignment_generation"], 2);
        assert!(
            app.works_account_dispatch(&request, "deliveries_handoff", handoff)
                .await
                .is_err()
        );
        let audit = app
            .db
            .query(Statement::new(
                "SELECT COUNT(*) AS n FROM works_handoff_audit",
                vec![],
            ))
            .await
            .unwrap();
        assert_eq!(integer(&audit[0], "n").unwrap(), 1);
        app.works_account_dispatch(&request,"sessions_update",json!({"assistant_id":"receiver","session_id":session,"status":"deleted","default_session_id":other["session_id"]})).await.unwrap();
        let assistants = app
            .db
            .query(Statement::new(
                "SELECT default_session_id FROM works_assistants WHERE assistant_id='receiver'",
                vec![],
            ))
            .await
            .unwrap();
        assert_eq!(
            text(&assistants[0], "default_session_id").unwrap(),
            other["session_id"].as_str().unwrap()
        );
        app.works_account_dispatch(&request,"sessions_update",json!({"assistant_id":"receiver","session_id":other["session_id"],"status":"deleted","default_session_id":null})).await.unwrap();
        let assistants = app
            .db
            .query(Statement::new(
                "SELECT default_session_id FROM works_assistants WHERE assistant_id='receiver'",
                vec![],
            ))
            .await
            .unwrap();
        assert_eq!(optional_text(&assistants[0], "default_session_id"), None);
        app.db
            .batch(vec![Statement::new(
                "UPDATE works_access_tokens SET expires_at=0 WHERE token_digest=?",
                vec![vec![7u8; 32].into()],
            )])
            .await
            .unwrap();
        assert!(app.works_sessions_dispatch(&receiver,"sessions_register",json!({"host_kind":"codex","external_session_id":"after-expiry","display_name":"Denied"})).await.is_err());
    }
}
