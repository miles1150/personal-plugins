//! A separate v2 notification journal. No legacy item is treated as a v2 delivery.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};

pub fn initialize(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS v2_wake_attempts (
        assistant_id TEXT NOT NULL, delivery_id TEXT NOT NULL, session_id TEXT NOT NULL,
        registration_revision INTEGER NOT NULL CHECK(registration_revision > 0),
        assignment_generation INTEGER NOT NULL CHECK(assignment_generation > 0),
        attempt_id TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('pending','sending','accepted','unknown','blocked')),
        queue_id TEXT, PRIMARY KEY (assistant_id, delivery_id, session_id, assignment_generation, attempt_id)
    );
    CREATE UNIQUE INDEX IF NOT EXISTS v2_delivery_once ON v2_wake_attempts(assistant_id,delivery_id,session_id,assignment_generation);
    CREATE TABLE IF NOT EXISTS v2_legacy_import (
        item_id TEXT PRIMARY KEY, resource_id TEXT NOT NULL, item_type TEXT NOT NULL,
        original_status TEXT NOT NULL, queue_id TEXT
    );
    UPDATE v2_wake_attempts SET status='unknown' WHERE status='sending';")?;
    Ok(())
}

/// Copy legacy references into a read-only quarantine. No mapping to a new delivery
/// can be inferred from an item ID, and unknown must not become a new pending wake.
pub fn import_legacy(db: &Connection) -> Result<usize> {
    Ok(db.execute(
        "INSERT OR IGNORE INTO v2_legacy_import
        SELECT item_id, resource_id, item_type,
        CASE WHEN status='sending' THEN 'unknown' ELSE status END, queue_id FROM items",
        [],
    )?)
}

/// Import an inactive legacy journal into the v2 state without modifying it.
pub fn import_file(db: &mut Connection, legacy_dir: &std::path::Path) -> Result<usize> {
    use fs2::FileExt;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let lock_path = legacy_dir.join("lock");
    super::private_file(&lock_path)?;
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(true);
    #[cfg(unix)]
    opts.custom_flags(libc::O_NOFOLLOW);
    let lock = opts.open(lock_path)?;
    lock.try_lock_exclusive()
        .context("stop the legacy bridge before importing its journal")?;
    let path = legacy_dir.join("state.sqlite3");
    super::private_file(&path)?;
    let old = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt =
        old.prepare("SELECT item_id,resource_id,item_type,status,queue_id FROM items")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    let tx = db.transaction()?;
    let mut count = 0;
    for row in rows {
        let (id, resource, kind, status, queue) = row?;
        let state = if status == "sending" {
            "unknown"
        } else {
            &status
        };
        count += tx.execute(
            "INSERT OR IGNORE INTO v2_legacy_import VALUES(?,?,?,?,?)",
            params![id, resource, kind, state, queue],
        )?;
    }
    tx.commit()?;
    Ok(count)
}

pub fn status(db: &Connection) -> Result<()> {
    let mut stmt = db
        .prepare("SELECT status, count(*) FROM v2_wake_attempts GROUP BY status ORDER BY status")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (status, count) = row?;
        println!("{status}: {count}");
    }
    let count: i64 = db.query_row("SELECT count(*) FROM v2_legacy_import", [], |r| r.get(0))?;
    println!("legacy references quarantined: {count}");
    Ok(())
}

pub fn resolve(db: &Connection, attempt: &str, result: &str) -> Result<()> {
    ensure!(
        ["accepted", "blocked"].contains(&result),
        "result must be accepted or blocked"
    );
    // Attempt IDs must be unique for operator resolution even across assistants.
    let count: i64 = db.query_row(
        "SELECT count(*) FROM v2_wake_attempts WHERE attempt_id=? AND status='unknown'",
        [attempt],
        |r| r.get(0),
    )?;
    ensure!(count == 1, "expected exactly one unknown attempt");
    ensure!(
        db.execute(
            "UPDATE v2_wake_attempts SET status=? WHERE attempt_id=? AND status='unknown'",
            params![result, attempt]
        )
        .context("resolve unknown attempt")?
            == 1,
        "attempt changed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_attempts_and_legacy_import_never_become_pending() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE items (item_id TEXT PRIMARY KEY, resource_id TEXT, item_type TEXT, status TEXT, queue_id TEXT);
            INSERT INTO items VALUES ('old','m','message','unknown',NULL);").unwrap();
        initialize(&db).unwrap();
        db.execute(
            "INSERT INTO v2_wake_attempts VALUES ('a','d','s',1,2,'attempt','sending',NULL)",
            [],
        )
        .unwrap();
        initialize(&db).unwrap();
        assert_eq!(import_legacy(&db).unwrap(), 1);
        assert_eq!(import_legacy(&db).unwrap(), 0);
        let status: String = db
            .query_row("SELECT status FROM v2_wake_attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "unknown");
        let status: String = db
            .query_row("SELECT original_status FROM v2_legacy_import", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "unknown");
        assert!(resolve(&db, "attempt", "pending").is_err());
        resolve(&db, "attempt", "accepted").unwrap();
        assert!(resolve(&db, "attempt", "accepted").is_err());
        let old: String = db
            .query_row("SELECT status FROM items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(old, "unknown");
    }
    #[cfg(unix)]
    #[test]
    fn imports_inactive_legacy_read_only_and_preserves_unknown() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("lock");
        std::fs::write(&lock, "").unwrap();
        std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o600)).unwrap();
        let path = dir.path().join("state.sqlite3");
        let old = Connection::open(&path).unwrap();
        old.execute_batch("CREATE TABLE items (item_id TEXT PRIMARY KEY,resource_id TEXT,item_type TEXT,status TEXT,queue_id TEXT); INSERT INTO items VALUES('item','message','message','sending',NULL);").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut fresh = Connection::open_in_memory().unwrap();
        initialize(&fresh).unwrap();
        assert_eq!(import_file(&mut fresh, dir.path()).unwrap(), 1);
        assert_eq!(import_file(&mut fresh, dir.path()).unwrap(), 0);
        let result: String = fresh
            .query_row("SELECT original_status FROM v2_legacy_import", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(result, "unknown");
        let untouched: String = old
            .query_row("SELECT status FROM items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(untouched, "sending");
    }
    #[test]
    fn ambiguous_attempt_id_cannot_be_resolved() {
        let db = Connection::open_in_memory().unwrap();
        initialize(&db).unwrap();
        db.execute(
            "INSERT INTO v2_wake_attempts VALUES ('a','d','s',1,1,'same','unknown',NULL)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO v2_wake_attempts VALUES ('a','e','s',1,1,'same','unknown',NULL)",
            [],
        )
        .unwrap();
        assert!(resolve(&db, "same", "accepted").is_err());
    }
}
