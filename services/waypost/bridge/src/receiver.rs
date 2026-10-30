//! Server-recorded proof that the fixed receiver used matching Waypost credentials.
//! Queue success proves only delivery, never receiver identity.
use super::{Adapter, AdapterOutcome, Config, PreparedAdapter, codex, oauth, request, webhook};
use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::Client;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Deserialize, Serialize)]
struct Pairing {
    request_id: String,
    target: String,
    binding_id: Option<String>,
    challenge: Option<String>,
    attempted: bool,
}
fn target(db: &Connection) -> Result<String> {
    let binding: String = db.query_row("SELECT value FROM meta WHERE key='binding'", [], |r| {
        r.get(0)
    })?;
    Ok(format!(
        "wpbridge_v1_{:x}",
        Sha256::digest(binding.as_bytes())
    ))
}
fn load(db: &Connection) -> Result<Option<Pairing>> {
    let text: Option<String> = db
        .query_row(
            "SELECT value FROM meta WHERE key='receiver_pairing'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    text.map(|s| serde_json::from_str(&s).context("invalid receiver pairing journal"))
        .transpose()
}
fn save(db: &Connection, pairing: &Pairing) -> Result<()> {
    db.execute("INSERT INTO meta(key,value) VALUES('receiver_pairing',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [serde_json::to_string(pairing)?])?;
    Ok(())
}
fn status(db: &Connection, client: &Client, c: &Config, p: &Pairing) -> Result<Value> {
    ensure!(
        p.target == target(db)?,
        "receiver target binding changed; use a separate state directory"
    );
    let id = p
        .binding_id
        .as_deref()
        .context("receiver challenge has not been created")?;
    ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "invalid receiver binding ID"
    );
    let (s, value) = request(
        client,
        c,
        "GET",
        &format!("/v0/receiver-bindings/{id}"),
        None,
    )?;
    ensure!(
        s == 200,
        "receiver verification status failed (HTTP {s}); delivery paused"
    );
    let data = &value["data"];
    ensure!(
        data["binding_id"] == id && data["target"] == p.target,
        "receiver proof target mismatch; delivery paused"
    );
    ensure!(
        ["pending", "verified", "expired"]
            .iter()
            .any(|s| data["status"] == *s),
        "invalid receiver verification response; delivery paused"
    );
    Ok(data.clone())
}

/// Sends at most one synthetic challenge per recorded pairing request.
pub fn pair(db: &Connection, client: &Client, c: &Config, renew: bool) -> Result<()> {
    let mut p = match if renew { None } else { load(db)? } {
        Some(pairing) => pairing,
        None => Pairing {
            request_id: format!("wppair_{}", oauth::random()?),
            target: target(db)?,
            binding_id: None,
            challenge: None,
            attempted: false,
        },
    };
    ensure!(
        p.target == target(db)?,
        "receiver target binding changed; use a separate state directory"
    );
    // Persist before creating the server record: retries reuse request_id.
    save(db, &p)?;
    if p.binding_id.is_none() {
        let (s, value) = request(
            client,
            c,
            "POST",
            "/v0/receiver-bindings",
            Some(json!({"request_id":p.request_id,"target":p.target})),
        )?;
        ensure!(
            s == 200 || s == 201,
            "receiver challenge creation failed (HTTP {s})"
        );
        let data = &value["data"];
        ensure!(
            data["target"] == p.target,
            "receiver challenge target mismatch"
        );
        let id = data["binding_id"]
            .as_str()
            .context("receiver binding ID missing")?;
        let challenge = data["challenge"]
            .as_str()
            .context("receiver challenge missing")?;
        ensure!(
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                && !challenge.is_empty()
                && challenge.len() <= 1024
                && !challenge.chars().any(char::is_control),
            "invalid receiver challenge response"
        );
        p.binding_id = Some(id.into());
        p.challenge = Some(challenge.into());
        save(db, &p)?;
    }
    let state = status(db, client, c, &p)?;
    if state["status"] == "verified" {
        println!(
            "Receiver credential identity verified by Waypost; proof checked with the service."
        );
        return Ok(());
    }
    ensure!(
        state["status"] != "expired",
        "receiver challenge expired; run pair-receiver --renew"
    );
    if p.attempted {
        println!(
            "Receiver verification pending. A challenge was already attempted; check the fixed receiver, or use pair-receiver --renew after independently checking the outcome."
        );
        return Ok(());
    }
    let id = p
        .binding_id
        .as_deref()
        .context("missing receiver binding")?;
    let challenge = p
        .challenge
        .as_deref()
        .context("missing receiver challenge")?;
    let prepared = match &c.adapter {
        Adapter::Codex(target) => {
            PreparedAdapter::Codex(codex::prepare_pairing(target, id, challenge)?)
        }
        Adapter::Webhook(target) => PreparedAdapter::Webhook(target.prepare_pairing(
            &c.token_file,
            webhook::Reference {
                origin: &c.base_url,
                space: &c.space_id,
                actor: &c.actor_id,
                consumer: &c.consumer_id,
                item: id,
                message: challenge,
            },
            &p.target,
        )?),
    };
    p.attempted = true;
    save(db, &p)?;
    match prepared.send() {
        AdapterOutcome::Accepted(_) => println!(
            "Receiver challenge accepted by the adapter; verification remains pending until the receiver confirms through its own Waypost connection."
        ),
        AdapterOutcome::Unknown => println!(
            "Receiver challenge delivery is unknown; verification remains pending. Check the fixed receiver before renewing."
        ),
        AdapterOutcome::NotAttempted => {
            p.attempted = false;
            save(db, &p)?;
            bail!("receiver challenge delivery not attempted; Codex could not start");
        }
    }
    Ok(())
}

pub fn verify(db: &Connection, client: &Client, c: &Config) -> Result<()> {
    if !c.require_receiver_verification {
        return Ok(());
    }
    match load(db)? {
        Some(p) if p.binding_id.is_some() => {
            let data = status(db, client, c, &p)?;
            if data["status"] == "verified" {
                return Ok(());
            }
            if data["status"] == "expired" {
                pair(db, client, c, true)?;
            }
        }
        _ => pair(db, client, c, false)?,
    }
    bail!(
        "receiver identity not verified; delivery paused until the fixed receiver confirms through its own Waypost connection"
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{db, send_pending};
    fn known(db: &Connection) -> Pairing {
        Pairing {
            request_id: "wppair_test".into(),
            target: target(db).unwrap(),
            binding_id: Some("receiver_test".into()),
            challenge: Some("opaque-challenge".into()),
            attempted: true,
        }
    }
    fn described() -> Value {
        json!({"data":{"space_id":"space","actor_id":"actor","consumer_id":"consumer"}})
    }
    #[test]
    fn receiver_proof_is_live_and_exactly_bound() {
        let (_dir, mut c) = crate::tests::fixture();
        c.require_receiver_verification = true;
        let (db, _lock) = db(&c).unwrap();
        let p = known(&db);
        save(&db, &p).unwrap();
        let (origin, handle) = crate::tests::server(vec![
            json!({"data":{"binding_id":"receiver_test","target":p.target,"status":"verified"}}),
            json!({"data":{"binding_id":"receiver_test","target":p.target,"status":"pending"}}),
            json!({"data":{"binding_id":"receiver_test","target":"other-host","status":"verified"}}),
        ]);
        c.base_url = origin;
        let client = Client::builder().no_proxy().build().unwrap();
        verify(&db, &client, &c).unwrap();
        assert!(
            verify(&db, &client, &c)
                .unwrap_err()
                .to_string()
                .contains("not verified")
        );
        assert!(
            verify(&db, &client, &c)
                .unwrap_err()
                .to_string()
                .contains("target mismatch")
        );
        assert_eq!(handle.join().unwrap().len(), 3);
    }
    #[test]
    fn pending_proof_sends_one_challenge_and_never_dispatches_message() {
        let (_dir, mut c) = crate::tests::fixture();
        c.require_receiver_verification = true;
        let (db, _lock) = db(&c).unwrap();
        let target = target(&db).unwrap();
        db.execute("INSERT INTO items(item_id,resource_id,item_type,status) VALUES('item','message','message','pending')",[]).unwrap();
        let (origin, handle) = crate::tests::server(vec![
            described(),
            json!({"data":{"binding_id":"receiver_test","target":target,"challenge":"opaque-challenge"}}),
            json!({"data":{"binding_id":"receiver_test","target":target,"status":"pending"}}),
            described(),
            json!({"data":{"binding_id":"receiver_test","target":target,"status":"pending"}}),
        ]);
        c.base_url = origin;
        let client = Client::builder().no_proxy().build().unwrap();
        assert!(send_pending(&db, &client, &c).is_err());
        assert!(load(&db).unwrap().unwrap().attempted);
        assert!(send_pending(&db, &client, &c).is_err());
        let status: String = db
            .query_row("SELECT status FROM items WHERE item_id='item'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "pending");
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 5);
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.starts_with("POST /v0/receiver-bindings "))
                .count(),
            1
        );
        assert!(requests.iter().all(|r| !r.starts_with("GET /v0/messages/")));
    }
    #[test]
    fn expired_proof_renews_once_and_pauses_dispatch() {
        let (_dir, mut c) = crate::tests::fixture();
        c.require_receiver_verification = true;
        let (db, _lock) = db(&c).unwrap();
        let p = known(&db);
        save(&db, &p).unwrap();
        let (origin, handle) = crate::tests::server(vec![
            json!({"data":{"binding_id":"receiver_test","target":p.target,"status":"expired"}}),
            json!({"data":{"binding_id":"receiver_new","target":p.target,"challenge":"new-challenge"}}),
            json!({"data":{"binding_id":"receiver_new","target":p.target,"status":"pending"}}),
            json!({"data":{"binding_id":"receiver_new","target":p.target,"status":"pending"}}),
        ]);
        c.base_url = origin;
        let client = Client::builder().no_proxy().build().unwrap();
        assert!(verify(&db, &client, &c).is_err());
        assert!(verify(&db, &client, &c).is_err());
        let renewed = load(&db).unwrap().unwrap();
        assert_eq!(renewed.binding_id.as_deref(), Some("receiver_new"));
        assert!(renewed.attempted);
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.starts_with("POST /v0/receiver-bindings "))
                .count(),
            1
        );
    }
    #[test]
    fn lost_pairing_send_does_not_repeat_when_pending() {
        let (_dir, mut c) = crate::tests::fixture();
        c.require_receiver_verification = true;
        let (db, _lock) = db(&c).unwrap();
        let p = known(&db);
        save(&db, &p).unwrap();
        if let Adapter::Codex(ref mut adapter) = c.adapter {
            adapter.codex_path = "/does/not/exist".into();
        }
        let (origin, handle) = crate::tests::server(vec![
            json!({"data":{"binding_id":"receiver_test","target":p.target,"status":"pending"}}),
        ]);
        c.base_url = origin;
        let client = Client::builder().no_proxy().build().unwrap();
        // An attempted challenge is not automatically retried even when no current CLI exists.
        pair(&db, &client, &c, false).unwrap();
        assert!(load(&db).unwrap().unwrap().attempted);
        handle.join().unwrap();
    }
}
