//! Fenced processing claims for cooperating receivers. A lease does not make
//! external business effects exactly once; callers must use request_id there.
use super::*;
use crate::domain::{check_id, check_request_id};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Claim {
    item_id: String,
    attempt_id: String,
    #[serde(default = "default_lease")]
    lease_seconds: i64,
}
fn default_lease() -> i64 {
    60
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Complete {
    item_id: String,
    attempt_id: String,
    generation: i64,
    request_id: String,
}

impl<D: Database, O: ObjectStore> App<D, O> {
    fn claim_key(&self, p: &Principal, item: &str) -> Vec<Sql> {
        vec![
            p.space_id.clone().into(),
            p.actor_id.clone().into(),
            p.consumer_id.clone().into(),
            item.into(),
        ]
    }

    async fn claim_visible_message(&self, p: &Principal, item: &str) -> Result<()> {
        self.current(p).await?;
        let rows = self
            .read_feed_rows(p, 0, 1, Some(&[item.to_owned()]))
            .await?;
        let row = rows.first().ok_or_else(Error::not_found)?;
        if text(row, "item_type")? != "message" {
            return Err(Error::invalid("Receiver claims require a message item."));
        }
        Ok(())
    }

    pub(crate) async fn receiver_claim(&self, p: &Principal, value: Value) -> Result<Value> {
        let input: Claim = crate::json::decode(value)?;
        check_id(&input.item_id)?;
        check_request_id(&input.attempt_id)?;
        if !(1..=300).contains(&input.lease_seconds) {
            return Err(Error::invalid("Lease must be between 1 and 300 seconds."));
        }
        self.claim_visible_message(p, &input.item_id).await?;
        let now = self.clock.now_ms();
        let expires = now
            .checked_add(input.lease_seconds * 1000)
            .ok_or_else(Error::storage)?;
        let key = self.claim_key(p, &input.item_id);
        let request_id = format!(
            "receiver:{}",
            hex::encode(Sha256::digest(
                serde_json::to_vec(&json!([
                    p.space_id,
                    p.actor_id,
                    p.consumer_id,
                    input.item_id
                ]))
                .map_err(|_| Error::storage())?
            ))
        );
        let (authority, auth) = self.authority_guard(p);
        let (visible, visibility) = self.feed_visibility(p, "f");
        let mut args = key.clone();
        args.extend([
            input.attempt_id.clone().into(),
            request_id.into(),
            expires.into(),
        ]);
        args.extend(auth.clone());
        args.extend([p.space_id.clone().into(), input.item_id.clone().into()]);
        args.extend(visibility);
        args.extend(key.clone());
        args.push(input.attempt_id.clone().into());
        args.extend([
            p.space_id.clone().into(),
            p.consumer_id.clone().into(),
            input.item_id.clone().into(),
            now.into(),
        ]);
        let mut history = vec![
            input.attempt_id.clone().into(),
            input.lease_seconds.into(),
            input.attempt_id.clone().into(),
        ];
        history.extend(key.clone());
        history.extend(auth);
        // Both statements execute in one transaction. The conditional UPSERT is
        // the only acquisition point; a preflight read never grants a lease.
        self.db.batch(vec![
            Statement::new(format!("INSERT INTO receiver_claims(space_id,actor_id,consumer_id,item_id,attempt_id,generation,request_id,lease_expires_at) SELECT ?,?,?,?,?,1,?,? WHERE {authority} AND EXISTS(SELECT 1 FROM feed_items f WHERE f.space_id=? AND f.item_id=? AND f.item_type='message' AND {visible}) AND NOT EXISTS(SELECT 1 FROM receiver_claim_attempts WHERE space_id=? AND actor_id=? AND consumer_id=? AND item_id=? AND attempt_id=?) AND NOT EXISTS(SELECT 1 FROM receipts WHERE space_id=? AND consumer_id=? AND subject_item_id=? AND stage=2) ON CONFLICT(space_id,actor_id,consumer_id,item_id) DO UPDATE SET attempt_id=excluded.attempt_id,generation=receiver_claims.generation+1,lease_expires_at=excluded.lease_expires_at WHERE receiver_claims.completed_at IS NULL AND receiver_claims.lease_expires_at<=? AND receiver_claims.generation<9007199254740991"), args),
            Statement::new(format!("INSERT INTO receiver_claim_attempts(space_id,actor_id,consumer_id,item_id,attempt_id,lease_seconds,generation,lease_expires_at,granted) SELECT space_id,actor_id,consumer_id,item_id,?,?,generation,lease_expires_at,attempt_id=? FROM receiver_claims WHERE space_id=? AND actor_id=? AND consumer_id=? AND item_id=? AND {authority} ON CONFLICT DO NOTHING"), history),
        ]).await?;
        self.current(p).await?;
        let mut args = key;
        args.push(input.attempt_id.into());
        let rows = self.db.query(Statement::new("SELECT a.*,c.generation AS current_generation,c.request_id,c.completed_at,EXISTS(SELECT 1 FROM receipts r WHERE r.space_id=c.space_id AND r.consumer_id=c.consumer_id AND r.subject_item_id=c.item_id AND r.stage=2) AS already_processed FROM receiver_claim_attempts a JOIN receiver_claims c USING(space_id,actor_id,consumer_id,item_id) WHERE a.space_id=? AND a.actor_id=? AND a.consumer_id=? AND a.item_id=? AND a.attempt_id=?", args)).await?;
        let Some(row) = rows.first() else {
            return Err(Error::new(
                "REVISION_CONFLICT",
                "Message is already processed or no longer claimable.",
            ));
        };
        if integer(row, "lease_seconds")? != input.lease_seconds {
            return Err(Error::new(
                "IDEMPOTENCY_CONFLICT",
                "Attempt ID was used with a different lease.",
            ));
        }
        let generation = integer(row, "generation")?;
        let current = generation == integer(row, "current_generation")?;
        let completed = integer(row, "already_processed")? == 1
            || (current && !matches!(row.get("completed_at"), None | Some(Sql::Null)));
        let granted = integer(row, "granted")? == 1;
        let active = current
            && granted
            && !completed
            && integer(row, "lease_expires_at")? > self.clock.now_ms();
        Ok(
            json!({"item_id":input.item_id,"acquired":active,"status":if completed {"completed"} else if active {"claimed"} else if granted {"expired"} else {"busy"},"generation":generation,"lease_expires_at":integer(row,"lease_expires_at")?,"request_id":text(row,"request_id")?}),
        )
    }

    pub(crate) async fn receiver_complete(&self, p: &Principal, value: Value) -> Result<Value> {
        let input: Complete = crate::json::decode(value)?;
        check_id(&input.item_id)?;
        check_request_id(&input.attempt_id)?;
        check_request_id(&input.request_id)?;
        if input.generation < 1 || input.generation as u64 > crate::domain::MAX_REVISION {
            return Err(Error::invalid("Invalid claim generation."));
        }
        self.claim_visible_message(p, &input.item_id).await?;
        let key = self.claim_key(p, &input.item_id);
        let rows = self.db.query(Statement::new("SELECT * FROM receiver_claims WHERE space_id=? AND actor_id=? AND consumer_id=? AND item_id=?", key.clone())).await?;
        let row = rows.first().ok_or_else(claim_lost)?;
        if text(row, "attempt_id")? != input.attempt_id
            || integer(row, "generation")? != input.generation
            || text(row, "request_id")? != input.request_id
        {
            return Err(claim_lost());
        }
        let fp = fingerprint(&json!({"item_id":input.item_id}))?;
        if !matches!(row.get("completed_at"), None | Some(Sql::Null)) {
            let submission = self
                .submission(p, "receiver.complete", &input.request_id)
                .await?
                .ok_or_else(Error::storage)?;
            self.existing_result(&submission, &fp)?;
            self.current(p).await?;
            return self.ack_result(&submission).await;
        }
        if integer(row, "lease_expires_at")? <= self.clock.now_ms() {
            return Err(claim_lost());
        }
        let reserved = self
            .reserve(
                p,
                "receiver.complete",
                &input.request_id,
                &fp,
                &p.consumer_id,
                None,
                &[],
            )
            .await?;
        let now = self.clock.now_ms();
        let sid = text(&reserved, "submission_id")?;
        let nonce = random_id("attempt")?;
        let (authority, auth) = self.authority_guard(p);
        let (visible, visibility) = self.feed_visibility(p, "f");
        let mut args = vec![nonce.clone().into(), sid.clone().into()];
        args.extend(auth);
        args.extend(key.clone());
        args.extend([
            input.attempt_id.into(),
            input.generation.into(),
            input.request_id.into(),
            now.into(),
            p.space_id.clone().into(),
            input.item_id.clone().into(),
        ]);
        args.extend(visibility);
        let mut q = vec![Statement::new(
            format!(
                "UPDATE submissions SET attempt_nonce=?,state='prepared' WHERE submission_id=? AND state IN('reserved','prepared') AND {authority} AND EXISTS(SELECT 1 FROM receiver_claims WHERE space_id=? AND actor_id=? AND consumer_id=? AND item_id=? AND attempt_id=? AND generation=? AND request_id=? AND completed_at IS NULL AND lease_expires_at>?) AND EXISTS(SELECT 1 FROM feed_items f WHERE f.space_id=? AND f.item_id=? AND {visible})"
            ),
            args,
        )];
        let guard = ("EXISTS(SELECT 1 FROM submissions WHERE submission_id=? AND attempt_nonce=? AND state='prepared')".to_owned(),vec![sid.clone().into(),nonce.clone().into()]);
        q.extend(push_rows("submission_ack_items","submission_id,space_id,position,subject_item_id,desired_stage,candidate_receipt_id,candidate_feed_id",vec![vec![sid.clone().into(),p.space_id.clone().into(),0i64.into(),input.item_id.into(),2i64.into(),random_id("receipt")?.into(),random_id("feed")?.into()]],Some(guard.clone())));
        for (sql, mut args) in [
            (
                crate::storage::queries::ACK_CREATE_RECEIPTS,
                vec![
                    p.consumer_id.clone().into(),
                    now.into(),
                    sid.clone().into(),
                    sid.clone().into(),
                ],
            ),
            (
                crate::storage::queries::ACK_ADVANCE_RECEIPTS,
                vec![
                    sid.clone().into(),
                    now.into(),
                    sid.clone().into(),
                    p.space_id.clone().into(),
                    p.consumer_id.clone().into(),
                    sid.clone().into(),
                ],
            ),
            (
                crate::storage::queries::ACK_PUBLISH_FEED,
                vec![
                    p.actor_id.clone().into(),
                    sid.clone().into(),
                    sid.clone().into(),
                ],
            ),
            (
                crate::storage::queries::ACK_SNAPSHOT_RECEIPTS,
                vec![p.actor_id.clone().into(), sid.clone().into()],
            ),
            (
                crate::storage::queries::ACK_DELIVER_RECEIPTS,
                vec![sid.clone().into()],
            ),
            (
                crate::storage::queries::ACK_SAVE_RESULTS,
                vec![sid.clone().into()],
            ),
        ] {
            args.extend(guard.1.clone());
            q.push(Statement::new(sql.replace("{guard}", &guard.0), args));
        }
        let mut args = vec![now.into()];
        args.extend(key.clone());
        args.extend(guard.1.clone());
        q.push(Statement::new(format!("UPDATE receiver_claims SET completed_at=? WHERE space_id=? AND actor_id=? AND consumer_id=? AND item_id=? AND {}",guard.0),args));
        q.push(Statement::new("UPDATE submissions SET state='committed',committed_at=? WHERE submission_id=? AND attempt_nonce=? AND state='prepared'",vec![now.into(),sid.into(),nonce.into()]));
        self.db.batch(q).await?;
        self.current(p).await?;
        // A different generation may have completed while this caller waited
        // for the transaction. Its committed business request is not our success.
        let final_claim = self.db.query(Statement::new("SELECT generation,attempt_id,completed_at FROM receiver_claims WHERE space_id=? AND actor_id=? AND consumer_id=? AND item_id=?",key)).await?;
        let final_claim = final_claim.first().ok_or_else(claim_lost)?;
        if integer(final_claim, "generation")? != input.generation
            || text(final_claim, "attempt_id")? != text(row, "attempt_id")?
            || matches!(final_claim.get("completed_at"), None | Some(Sql::Null))
        {
            return Err(claim_lost());
        }
        let result = self
            .submission(p, "receiver.complete", &text(row, "request_id")?)
            .await?
            .ok_or_else(Error::storage)?;
        if text(&result, "state")? != "committed" {
            return Err(claim_lost());
        }
        self.ack_result(&result).await
    }
}

fn claim_lost() -> Error {
    Error::new(
        "REVISION_CONFLICT",
        "Receiver claim is expired or superseded; stop processing and reconcile effects.",
    )
}

#[cfg(all(test, feature = "native"))]
mod tests;
