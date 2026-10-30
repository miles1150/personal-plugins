use super::*;
use crate::native::{FileStore, SqliteDb};
use std::cell::{Cell, RefCell};

struct TestClock(Cell<i64>);
impl Clock for TestClock {
    fn now_ms(&self) -> i64 {
        self.0.get()
    }
}

struct GateDb {
    inner: SqliteDb,
    gate: Cell<bool>,
    complete_gate: Cell<bool>,
    arrivals: Cell<usize>,
    waiting: RefCell<Option<std::task::Waker>>,
    before_complete: RefCell<Vec<Statement>>,
}
impl Database for GateDb {
    async fn query(&self, s: Statement) -> Result<Vec<Row>> {
        self.inner.query(s).await
    }
    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Vec<Row>>> {
        let first = statements.first();
        if (self.gate.get()
            && first.is_some_and(|s| s.sql.starts_with("INSERT INTO receiver_claims")))
            || (self.complete_gate.get()
                && first
                    .is_some_and(|s| s.sql.starts_with("UPDATE submissions SET attempt_nonce=")))
        {
            self.arrivals.set(self.arrivals.get() + 1);
            futures_util::future::poll_fn(|cx| {
                if self.arrivals.get() >= 2 {
                    if let Some(w) = self.waiting.borrow_mut().take() {
                        w.wake();
                    }
                    std::task::Poll::Ready(())
                } else {
                    *self.waiting.borrow_mut() = Some(cx.waker().clone());
                    std::task::Poll::Pending
                }
            })
            .await;
        }
        if statements
            .first()
            .is_some_and(|s| s.sql.starts_with("UPDATE submissions SET attempt_nonce="))
        {
            let inject = self
                .before_complete
                .borrow_mut()
                .drain(..)
                .collect::<Vec<_>>();
            if !inject.is_empty() {
                self.inner.batch(inject).await?;
            }
        }
        self.inner.batch(statements).await
    }
}

async fn fixture() -> (
    tempfile::TempDir,
    App<GateDb, FileStore>,
    Principal,
    String,
    Rc<TestClock>,
) {
    let dir = tempfile::tempdir().unwrap();
    let clock = Rc::new(TestClock(Cell::new(1_800_000_000_000)));
    let app = App::new(
        GateDb {
            inner: SqliteDb::open(dir.path().join("db")).unwrap(),
            gate: Cell::new(false),
            complete_gate: Cell::new(false),
            arrivals: Cell::new(0),
            waiting: RefCell::new(None),
            before_complete: RefCell::new(vec![]),
        },
        FileStore::open(dir.path().join("objects")).unwrap(),
        Config {
            admin_secret: "claim-tests-admin-secret-32-bytes".into(),
            public_origin: "http://localhost".into(),
        },
    )
    .with_clock(clock.clone());
    let body = json!({"request_id":"setup","display_name":"Test"});
    let space = app
        .admin(
            &Request::json(
                "POST",
                "/admin/v0/spaces",
                &app.config.admin_secret,
                body.clone(),
            ),
            body,
        )
        .await
        .unwrap();
    let path = format!(
        "/admin/v0/spaces/{}/tokens",
        space["space_id"].as_str().unwrap()
    );
    let body = json!({"request_id":"token","actor_id":space["default_actor_id"],"consumer_id":space["default_consumer_id"]});
    let issued = app
        .admin(
            &Request::json("POST", &path, &app.config.admin_secret, body.clone()),
            body,
        )
        .await
        .unwrap();
    let p = app
        .authenticate(&Request::json(
            "GET",
            "/v0/describe",
            issued["token"].as_str().unwrap(),
            json!({}),
        ))
        .await
        .unwrap();
    app.send(&p,json!({"request_id":"message","recipient_ids":[p.actor_id],"kind":"info","subject":"Test","body":{"text":"Work"}})).await.unwrap();
    let rows = app
        .db
        .query(Statement::new(
            "SELECT item_id FROM feed_items WHERE item_type='message'",
            vec![],
        ))
        .await
        .unwrap();
    let item = text(&rows[0], "item_id").unwrap();
    (dir, app, p, item, clock)
}
fn completion(item: &str, attempt: &str, claim: &Value) -> Value {
    json!({"item_id":item,"attempt_id":attempt,"generation":claim["generation"],"request_id":claim["request_id"]})
}

#[tokio::test(flavor = "current_thread")]
async fn simultaneous_claims_have_one_winner_and_no_receipt() {
    let (_dir, app, p, item, _clock) = fixture().await;
    app.db.gate.set(true);
    let (a, b) = futures_util::join!(
        app.receiver_claim(&p, json!({"item_id":item,"attempt_id":"a"})),
        app.receiver_claim(&p, json!({"item_id":item,"attempt_id":"b"}))
    );
    assert_eq!(app.db.arrivals.get(), 2);
    let a = a.unwrap();
    let b = b.unwrap();
    assert_ne!(a["acquired"], b["acquired"]);
    assert_eq!(a["generation"], 1);
    assert_eq!(b["generation"], 1);
    assert_eq!(a["request_id"], b["request_id"]);
    assert!(
        app.db
            .query(Statement::new("SELECT * FROM receipts", vec![]))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn simultaneous_retries_share_one_claim_and_one_completion() {
    let (_dir, app, p, item, _clock) = fixture().await;
    app.db.gate.set(true);
    let input = json!({"item_id":item,"attempt_id":"same-attempt"});
    let (a, b) = futures_util::join!(
        app.receiver_claim(&p, input.clone()),
        app.receiver_claim(&p, input)
    );
    let a = a.unwrap();
    assert_eq!(a, b.unwrap());
    assert_eq!(a["acquired"], true);
    app.db.gate.set(false);
    app.db.complete_gate.set(true);
    app.db.arrivals.set(0);
    let input = completion(&item, "same-attempt", &a);
    let (a, b) = futures_util::join!(
        app.receiver_complete(&p, input.clone()),
        app.receiver_complete(&p, input)
    );
    assert_eq!(app.db.arrivals.get(), 2);
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(
        app.db
            .query(Statement::new("SELECT * FROM receipts", vec![]))
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        app.db
            .query(Statement::new("SELECT * FROM feed_receipt_updates", vec![]))
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test(flavor = "current_thread")]
async fn retry_expiry_takeover_and_completion_are_fenced_and_idempotent() {
    let (_dir, app, p, item, clock) = fixture().await;
    let original = json!({"item_id":item,"attempt_id":"a","lease_seconds":1});
    let a = app.receiver_claim(&p, original.clone()).await.unwrap();
    assert_eq!(a, app.receiver_claim(&p, original.clone()).await.unwrap());
    assert_eq!(
        app.receiver_claim(
            &p,
            json!({"item_id":item,"attempt_id":"a","lease_seconds":2})
        )
        .await
        .unwrap_err()
        .code,
        "IDEMPOTENCY_CONFLICT"
    );
    let denied = json!({"item_id":item,"attempt_id":"denied"});
    assert_eq!(
        app.receiver_claim(&p, denied.clone()).await.unwrap()["acquired"],
        false
    );
    clock.0.set(clock.now_ms() + 1000);
    assert_eq!(
        app.receiver_claim(&p, original.clone()).await.unwrap()["status"],
        "expired"
    );
    assert_eq!(
        app.receiver_complete(&p, completion(&item, "a", &a))
            .await
            .unwrap_err()
            .code,
        "REVISION_CONFLICT"
    );
    // Replaying a previously denied attempt cannot later acquire the work.
    assert_eq!(
        app.receiver_claim(&p, denied).await.unwrap()["acquired"],
        false
    );
    let b = app
        .receiver_claim(&p, json!({"item_id":item,"attempt_id":"b"}))
        .await
        .unwrap();
    assert_eq!(b["generation"], 2);
    assert_eq!(b["request_id"], a["request_id"]);
    assert_eq!(
        app.receiver_complete(&p, completion(&item, "a", &a))
            .await
            .unwrap_err()
            .code,
        "REVISION_CONFLICT"
    );
    let complete = completion(&item, "b", &b);
    let first = app.receiver_complete(&p, complete.clone()).await.unwrap();
    assert_eq!(first["items"][0]["stage"], "processed");
    let status = app
        .status(
            &p,
            json!({"operation":"receiver.complete","request_id":b["request_id"]}),
        )
        .await
        .unwrap();
    assert_eq!(status["submission_state"], "committed");
    assert_eq!(status["result"], first);
    clock.0.set(clock.now_ms() + 400_000);
    assert_eq!(first, app.receiver_complete(&p, complete).await.unwrap());
    assert_eq!(
        app.receiver_claim(&p, json!({"item_id":item,"attempt_id":"c"}))
            .await
            .unwrap()["acquired"],
        false
    );
    let rows = app
        .db
        .query(Statement::new("SELECT * FROM receipts", vec![]))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(integer(&rows[0], "stage").unwrap(), 2);
    assert_eq!(integer(&rows[0], "version").unwrap(), 1);
    assert_eq!(
        app.db
            .query(Statement::new("SELECT * FROM feed_receipt_updates", vec![]))
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test(flavor = "current_thread")]
async fn takeover_at_completion_transaction_blocks_stale_receipt() {
    let (_dir, app, p, item, _clock) = fixture().await;
    let a = app
        .receiver_claim(&p, json!({"item_id":item,"attempt_id":"a"}))
        .await
        .unwrap();
    app.db.before_complete.borrow_mut().push(Statement::new("UPDATE receiver_claims SET generation=generation+1,attempt_id='new-worker' WHERE item_id=?",vec![item.clone().into()]));
    assert_eq!(
        app.receiver_complete(&p, completion(&item, "a", &a))
            .await
            .unwrap_err()
            .code,
        "REVISION_CONFLICT"
    );
    assert!(
        app.db
            .query(Statement::new("SELECT * FROM receipts", vec![]))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        app.db
            .query(Statement::new("SELECT * FROM submission_ack_items", vec![]))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn revoked_authority_at_completion_blocks_processed_receipt() {
    let (_dir, app, p, item, _clock) = fixture().await;
    let a = app
        .receiver_claim(&p, json!({"item_id":item,"attempt_id":"a"}))
        .await
        .unwrap();
    app.db.before_complete.borrow_mut().push(Statement::new(
        "UPDATE tokens SET revoked_at=1 WHERE token_id=?",
        vec![p.token_id.clone().into()],
    ));
    assert_eq!(
        app.receiver_complete(&p, completion(&item, "a", &a))
            .await
            .unwrap_err()
            .code,
        "UNAUTHENTICATED"
    );
    assert!(
        app.db
            .query(Statement::new("SELECT * FROM receipts", vec![]))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn received_remains_received_until_completion_and_old_processed_is_respected() {
    let (_dir, app, p, item, _clock) = fixture().await;
    app.ack(&p,json!({"request_id":"received","consumer_id":p.consumer_id,"items":[{"item_id":item,"stage":"received"}]})).await.unwrap();
    let a = app
        .receiver_claim(&p, json!({"item_id":item,"attempt_id":"a"}))
        .await
        .unwrap();
    assert_eq!(a["acquired"], true);
    let rows = app
        .db
        .query(Statement::new("SELECT stage,version FROM receipts", vec![]))
        .await
        .unwrap();
    assert_eq!(integer(&rows[0], "stage").unwrap(), 1);
    assert_eq!(integer(&rows[0], "version").unwrap(), 1);
    let done = app
        .receiver_complete(&p, completion(&item, "a", &a))
        .await
        .unwrap();
    assert_eq!(done["items"][0]["version"], 2);
    assert_eq!(
        app.db
            .query(Statement::new("SELECT * FROM feed_receipt_updates", vec![]))
            .await
            .unwrap()
            .len(),
        2
    );

    let (_dir, app, p, item, _clock) = fixture().await;
    let a = app
        .receiver_claim(&p, json!({"item_id":item,"attempt_id":"a"}))
        .await
        .unwrap();
    // Old consumers remain compatible and can bypass a processing claim.
    app.ack(&p,json!({"request_id":"legacy-processed","consumer_id":p.consumer_id,"items":[{"item_id":item,"stage":"processed"}]})).await.unwrap();
    let retry = app
        .receiver_claim(&p, json!({"item_id":item,"attempt_id":"a"}))
        .await
        .unwrap();
    assert_eq!(retry["acquired"], false);
    assert_eq!(retry["status"], "completed");
    let done = app
        .receiver_complete(&p, completion(&item, "a", &a))
        .await
        .unwrap();
    assert_eq!(done["items"][0]["version"], 1);
    assert_eq!(
        app.db
            .query(Statement::new("SELECT * FROM feed_receipt_updates", vec![]))
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test(flavor = "current_thread")]
async fn invisible_items_and_invalid_leases_do_not_create_claims() {
    let (_dir, app, p, item, _clock) = fixture().await;
    for lease in [0, 301] {
        assert_eq!(
            app.receiver_claim(
                &p,
                json!({"item_id":item,"attempt_id":"bad","lease_seconds":lease})
            )
            .await
            .unwrap_err()
            .code,
            "INVALID_ARGUMENT"
        );
    }
    assert_eq!(
        app.receiver_claim(&p, json!({"item_id":"feed_missing","attempt_id":"missing"}))
            .await
            .unwrap_err()
            .code,
        "NOT_FOUND"
    );
    app.db
        .batch(vec![Statement::new(
            "DELETE FROM feed_recipients WHERE item_id=?",
            vec![item.clone().into()],
        )])
        .await
        .unwrap();
    assert_eq!(
        app.receiver_claim(&p, json!({"item_id":item,"attempt_id":"hidden"}))
            .await
            .unwrap_err()
            .code,
        "NOT_FOUND"
    );
    assert!(
        app.db
            .query(Statement::new("SELECT * FROM receiver_claims", vec![]))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        app.db
            .query(Statement::new(
                "SELECT * FROM receiver_claim_attempts",
                vec![]
            ))
            .await
            .unwrap()
            .is_empty()
    );
}
