//! Encrypted, message-scoped attachment reservations.
use super::works_v2::WorksPrincipal;
use super::*;

fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::invalid("Missing attachment field."))
}
impl<D: Database, O: ObjectStore> App<D, O> {
    /// Internal bounded sweep. Tombstones remain eligible for re-sweep so a
    /// stalled object write cannot leave a permanent orphan after expiry.
    pub(crate) async fn works_attachments_gc(&self) -> Result<usize> {
        let now = self.clock.now_ms();
        self.db.batch(vec![Statement::new("UPDATE works_message_attachments SET state='expired' WHERE attachment_id IN (SELECT attachment_id FROM works_message_attachments WHERE state IN ('pending','uploaded') AND expires_at<=? ORDER BY expires_at,attachment_id LIMIT 32)",vec![now.into()])]).await?;
        let rows=self.db.query(Statement::new("SELECT attachment_id,object_key FROM works_message_attachments WHERE state='expired' ORDER BY COALESCE(cleanup_checked_at,0),attachment_id LIMIT 32",vec![])).await?;
        for row in &rows {
            let id = text(row, "attachment_id")?;
            self.objects.remove(&text(row, "object_key")?).await?;
            self.db.batch(vec![Statement::new("UPDATE works_message_attachments SET cleanup_checked_at=? WHERE attachment_id=? AND state='expired'",vec![now.into(),id.into()])]).await?;
        }
        Ok(rows.len())
    }
    pub(super) async fn works_attachments_dispatch(
        &self,
        p: &WorksPrincipal,
        op: &str,
        v: Value,
    ) -> Result<Value> {
        if !p.messages_enabled {
            return Err(Error::forbidden());
        }
        match op {
            "attachments_prepare" => self.works_attachment_prepare(p, v).await,
            "attachments_upload" => self.works_attachment_upload(p, v).await,
            "attachments_read" => self.works_attachment_read(p, v).await,
            _ => Err(Error::not_found()),
        }
    }
    async fn works_attachment_prepare(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        if self.encryption.is_none() {
            return Err(Error::new(
                "ENCRYPTION_REQUIRED",
                "Attachment encryption is not configured.",
            ));
        }
        let request = field(&v, "request_id")?;
        crate::domain::check_request_id(request)?;
        let ct = field(&v, "content_type")?;
        if ct.len() > 128 || !ct.contains('/') || ct.bytes().any(|b| b < 32 || b == 127) {
            return Err(Error::invalid("Invalid content type."));
        }
        let size = v
            .get("size_bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::invalid("Invalid attachment size."))?;
        // MCP requests are bounded JSON. Larger files require a future binary upload route.
        let max = (self.limits.max_json_bytes.saturating_sub(4096) * 3 / 4) as u64;
        if size > self.limits.max_blob_bytes || size > max {
            return Err(Error::new(
                "RESOURCE_LIMIT",
                "Attachment exceeds the inline upload limit.",
            ));
        }
        let hash = field(&v, "sha256")?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::invalid("Invalid SHA-256."));
        }
        let name = v.get("filename").and_then(Value::as_str);
        if name.is_some_and(|n| n.len() > 256 || n.chars().any(char::is_control)) {
            return Err(Error::invalid("Invalid filename."));
        }
        let id = random_id("attachment")?;
        let object = self.storage_key(&p.account_id, &format!("works-attachment:{id}"));
        let filename = name
            .map(|s| self.seal_field(&p.account_id, &format!("works-attachment:{id}:filename"), s))
            .transpose()?;
        let fp = fingerprint(&json!([ct, size, hash, name]))?;
        let (g, ga) = self.works_v2_guard(p);
        if self
            .db
            .query(Statement::new(format!("SELECT 1 WHERE {g}"), ga))
            .await?
            .is_empty()
        {
            return Err(Error::unauthenticated());
        }
        self.works_attachments_gc().await?;
        let now = self.clock.now_ms();
        let (g, ga) = self.works_v2_guard(p);
        let mut a = vec![
            id.into(),
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            request.into(),
            fp.clone().into(),
            object.into(),
            ct.into(),
            filename.into(),
            (size as i64).into(),
            hash.into(),
            now.into(),
            (now + 3_600_000).into(),
        ];
        a.extend(ga.clone());
        self.db.batch(vec![Statement::new(format!("INSERT INTO works_message_attachments(attachment_id,account_id,sender_assistant_id,request_id,fingerprint,object_key,content_type,filename_ciphertext,size_bytes,sha256,state,created_at,expires_at) SELECT ?,?,?,?,?,?,?,?,?,?,'pending',?,? WHERE {g} ON CONFLICT(account_id,sender_assistant_id,request_id) DO NOTHING"),a)]).await?;
        let mut a = vec![
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            request.into(),
        ];
        a.extend(ga);
        let rows=self.db.query(Statement::new(format!("SELECT attachment_id,fingerprint,state,expires_at FROM works_message_attachments WHERE account_id=? AND sender_assistant_id=? AND request_id=? AND {g}"),a)).await?;
        let row = rows.first().ok_or_else(Error::unauthenticated)?;
        if text(row, "fingerprint")? != fp {
            return Err(Error::new(
                "IDEMPOTENCY_CONFLICT",
                "Reservation request ID already used.",
            ));
        }
        if text(row, "state")? == "expired" {
            return Err(Error::new(
                "ATTACHMENT_EXPIRED",
                "Attachment reservation expired.",
            ));
        }
        Ok(
            json!({"attachment_id":text(row,"attachment_id")?,"state":text(row,"state")?,"expires_at":integer(row,"expires_at")?,"max_inline_bytes":max}),
        )
    }
    async fn works_attachment_upload(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        let id = field(&v, "attachment_id")?;
        crate::domain::check_id(id)?;
        let encoded = v
            .get("content_base64")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::invalid("Missing attachment content."))?;
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| Error::invalid("Invalid base64url content."))?;
        if bytes.len() > self.limits.max_json_bytes {
            return Err(Error::new("RESOURCE_LIMIT", "Attachment too large."));
        }
        let now = self.clock.now_ms();
        let (g, ga) = self.works_v2_guard(p);
        let mut a = vec![
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            id.into(),
            now.into(),
        ];
        a.extend(ga.clone());
        let rows=self.db.query(Statement::new(format!("SELECT * FROM works_message_attachments WHERE account_id=? AND sender_assistant_id=? AND attachment_id=? AND state IN ('pending','uploaded') AND expires_at>? AND {g}"),a)).await?;
        let r = rows.first().ok_or_else(Error::not_found)?;
        let hash = hex::encode(Sha256::digest(&bytes));
        if bytes.len() as i64 != integer(r, "size_bytes")? || hash != text(r, "sha256")? {
            return Err(Error::new(
                "CHECKSUM_MISMATCH",
                "Attachment size or checksum does not match.",
            ));
        }
        self.object_create(
            &p.account_id,
            &text(r, "object_key")?,
            &bytes,
            &text(r, "content_type")?,
            &Sha256::digest(&bytes),
        )
        .await?;
        let (g, ga) = self.works_v2_guard(p);
        let mut a = vec![
            p.account_id.clone().into(),
            p.assistant_id.clone().into(),
            id.into(),
            self.clock.now_ms().into(),
        ];
        a.extend(ga);
        let result=self.db.batch(vec![Statement::new(format!("UPDATE works_message_attachments SET state='uploaded' WHERE account_id=? AND sender_assistant_id=? AND attachment_id=? AND state IN ('pending','uploaded') AND expires_at>? AND {g} RETURNING attachment_id"),a)]).await?;
        if result[0].is_empty() {
            return Err(Error::new(
                "REVISION_CONFLICT",
                "Attachment reservation is no longer valid.",
            ));
        }
        Ok(json!({"attachment_id":id,"state":"uploaded"}))
    }
    async fn works_attachment_read(&self, p: &WorksPrincipal, v: Value) -> Result<Value> {
        let id = field(&v, "attachment_id")?;
        let message = field(&v, "message_id")?;
        let (g, ga) = self.works_v2_guard(p);
        let mut a = vec![
            p.account_id.clone().into(),
            id.into(),
            message.into(),
            p.assistant_id.clone().into(),
            p.assistant_id.clone().into(),
        ];
        a.extend(ga.clone());
        let rows=self.db.query(Statement::new(format!("SELECT b.* FROM works_message_attachments b JOIN works_messages m ON m.account_id=b.account_id AND m.message_id=b.message_id WHERE b.account_id=? AND b.attachment_id=? AND b.message_id=? AND b.state='linked' AND (m.sender_assistant_id=? OR EXISTS(SELECT 1 FROM works_deliveries d WHERE d.account_id=m.account_id AND d.message_id=m.message_id AND d.assistant_id=?)) AND {g}"),a)).await?;
        let row = rows.first().ok_or_else(Error::not_found)?;
        let bytes = self
            .object_read(&p.account_id, &text(row, "object_key")?)
            .await?;
        let (g, ga) = self.works_v2_guard(p);
        let check = self
            .db
            .query(Statement::new(format!("SELECT 1 WHERE {g}"), ga))
            .await?;
        if check.is_empty() {
            return Err(Error::unauthenticated());
        }
        let filename = match crate::storage::driver::optional_text(row, "filename_ciphertext") {
            Some(n) => Some(self.open_field(
                &p.account_id,
                &format!("works-attachment:{id}:filename"),
                &n,
            )?),
            None => None,
        };
        Ok(
            json!({"attachment_id":id,"message_id":message,"content_type":text(row,"content_type")?,"filename":filename,"size_bytes":integer(row,"size_bytes")?,"sha256":text(row,"sha256")?,"content_base64":URL_SAFE_NO_PAD.encode(bytes)}),
        )
    }
}
