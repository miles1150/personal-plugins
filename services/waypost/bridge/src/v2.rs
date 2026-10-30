//! Installation-level v2 supervisor. It never accesses another host's tokens.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    config_version: u32,
    base_url: String,
    state_dir: PathBuf,
    oauth: oauth::OAuthConfig,
    sessions: Vec<Session>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    assistant_id: String,
    session_id: String,
    thread_id: String,
    codex_path: PathBuf,
    codex_remote: Option<String>,
    codex_home: Option<PathBuf>,
}
impl Session {
    fn codex(&self) -> codex::CodexConfig {
        codex::CodexConfig {
            thread_id: self.thread_id.clone(),
            codex_path: self.codex_path.clone(),
            codex_remote: self.codex_remote.clone(),
            codex_home: self.codex_home.clone(),
        }
    }
    fn target(&self, origin: &str) -> String {
        format!(
            "wptarget_{}",
            hex_sha(
                &json!([
                    origin,
                    self.assistant_id,
                    self.session_id,
                    self.thread_id,
                    self.codex_path,
                    self.codex_remote,
                    self.codex_home
                ])
                .to_string()
            )
        )
    }
}
fn hex_sha(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn settings(path: &Path) -> Result<(Settings, Config)> {
    let s: Settings = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        s.config_version == 3 && !s.sessions.is_empty(),
        "v2 requires at least one session"
    );
    let url = url::Url::parse(&s.base_url)?;
    let loopback = url.host_str().is_some_and(|h| {
        h == "localhost" || h.parse::<std::net::IpAddr>().is_ok_and(|a| a.is_loopback())
    });
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && (url.path() == "/" || url.path().is_empty())
            && (url.scheme() == "https" || url.scheme() == "http" && loopback),
        "invalid service origin"
    );
    ensure!(s.state_dir.is_absolute(), "state_dir must be absolute");
    s.oauth.validate()?;
    let uuid = Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")?;
    let mut seen = std::collections::HashSet::new();
    for x in &s.sessions {
        ensure!(
            id(&x.assistant_id)
                && id(&x.session_id)
                && uuid.is_match(&x.thread_id)
                && x.codex_path.is_absolute()
                && x.codex_home.as_ref().is_none_or(|p| p.is_absolute()),
            "invalid session configuration"
        );
        if let Some(r) = &x.codex_remote {
            ensure!(
                r.starts_with("unix:///")
                    && !r.contains(['?', '#', '\n', '\r'])
                    && Path::new(&r[7..]).components().all(|p| matches!(
                        p,
                        std::path::Component::RootDir | std::path::Component::Normal(_)
                    )),
                "invalid Codex socket"
            );
        }
        ensure!(seen.insert(&x.session_id), "duplicate session");
    }
    // Reuse the existing private DB lock and OAuth implementation. This is an
    // independent bridge installation; no host credential is consulted.
    let first = s.sessions[0].codex();
    let c = Config {
        base_url: s.base_url.clone(),
        v2: true,
        token_file: s.oauth.credentials_file.clone(),
        oauth: Some(oauth::OAuthConfig {
            credentials_file: s.oauth.credentials_file.clone(),
            client_id: s.oauth.client_id.clone(),
            redirect_port: s.oauth.redirect_port,
        }),
        require_receiver_verification: true,
        state_dir: s.state_dir.clone(),
        space_id: "v2".into(),
        actor_id: "v2".into(),
        consumer_id: "v2".into(),
        adapter: Adapter::Codex(first),
    };
    Ok((s, c))
}
fn call(client: &Client, c: &Config, name: &str, args: Value) -> Result<Value> {
    let token = oauth::access_token(client, c)?;
    let response=client.post(format!("{}/mcp",c.base_url.trim_end_matches('/'))).bearer_auth(token).json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}})).send().context("MCP request failed")?;
    let status = response.status();
    ensure!(status.is_success(), "MCP request failed (HTTP {status})");
    let mut bytes = Vec::new();
    response.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 2 * 1024 * 1024, "MCP response too large");
    let v: Value = serde_json::from_slice(&bytes)?;
    let result = &v["result"]["structuredContent"];
    ensure!(
        result["ok"] == true,
        "MCP tool {name} failed: {}",
        result["error"]["code"]
            .as_str()
            .unwrap_or("invalid response")
    );
    Ok(result["data"].clone())
}
fn identity(db: &Connection, client: &Client, c: &Config) -> Result<()> {
    let v = call(client, c, "works_identity", json!({}))?;
    let installation = v["installation_id"]
        .as_str()
        .filter(|s| id(s))
        .context("missing bridge installation")?;
    let old: Option<String> = db
        .query_row(
            "SELECT value FROM meta WHERE key='v2_installation'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    ensure!(
        old.as_deref().is_none_or(|x| x == installation),
        "bridge installation changed; preserve the journal and explicitly pair the new installation"
    );
    db.execute(
        "INSERT OR IGNORE INTO meta VALUES ('v2_installation',?)",
        [installation],
    )?;
    Ok(())
}
fn pair(db: &Connection, client: &Client, c: &Config, x: &Session) -> Result<()> {
    let target = x.target(&c.base_url);
    let existing: Option<(String, String)> = db
        .query_row(
            "SELECT binding_id,target FROM v2_pairs WHERE session_id=?",
            [&x.session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((binding, old)) = existing {
        ensure!(
            old == target,
            "session adapter changed; preserve the journal and create a new pairing explicitly"
        );
        let v = call(
            client,
            c,
            "works_wake_status",
            json!({"assistant_id":x.assistant_id,"binding_id":binding}),
        )?;
        println!(
            "{}: {}",
            x.session_id,
            v["status"].as_str().unwrap_or("unknown")
        );
        return Ok(());
    }
    let req = oauth::random()?;
    let v = call(
        client,
        c,
        "works_wake_create",
        json!({"assistant_id":x.assistant_id,"session_id":x.session_id,"request_id":req,"target":target}),
    )?;
    let binding = v["binding_id"]
        .as_str()
        .filter(|s| id(s))
        .context("missing wake binding")?;
    let challenge = v["challenge"]
        .as_str()
        .filter(|s| id(s))
        .context("missing wake challenge")?;
    let revision = v["registration_revision"]
        .as_i64()
        .filter(|n| *n > 0)
        .context("missing registration revision")?;
    let adapter = x.codex();
    let prepared =
        codex::prepare_pairing_v2(&adapter, &x.assistant_id, &x.session_id, binding, challenge)?;
    db.execute("INSERT INTO v2_pairs(session_id,assistant_id,binding_id,target,registration_revision,status) VALUES(?,?,?,?,?,'sending')",params![x.session_id,x.assistant_id,binding,target,revision])?;
    let status = match prepared.send() {
        AdapterOutcome::Accepted(_) => "sent",
        AdapterOutcome::Unknown => "unknown",
        AdapterOutcome::NotAttempted => "not_attempted",
    };
    db.execute(
        "UPDATE v2_pairs SET status=? WHERE session_id=?",
        params![status, x.session_id],
    )?;
    println!(
        "{}: pairing {status}; receiver must confirm using its own connection",
        x.session_id
    );
    Ok(())
}
fn scan(db: &mut Connection, client: &Client, c: &Config, x: &Session) -> Result<()> {
    let row:Option<(String,String,i64)>=db.query_row("SELECT binding_id,target,registration_revision FROM v2_pairs WHERE session_id=? AND assistant_id=?",params![x.session_id,x.assistant_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let Some((binding, target, revision)) = row else {
        bail!("session {} is not paired", x.session_id)
    };
    ensure!(
        target == x.target(&c.base_url),
        "session adapter differs from pairing"
    );
    let mut cursor: Option<String> = None;
    loop {
        let v = call(
            client,
            c,
            "works_wake_poll",
            json!({"assistant_id":x.assistant_id,"binding_id":binding,"cursor":cursor}),
        )?;
        let items = v["items"].as_array().context("invalid wake page")?;
        let tx = db.transaction()?;
        for item in items {
            let delivery = item["delivery_id"]
                .as_str()
                .filter(|s| id(s))
                .context("invalid delivery")?;
            let message = item["message_id"]
                .as_str()
                .filter(|s| id(s))
                .context("invalid message")?;
            ensure!(
                item["session_id"] == x.session_id,
                "wrong session in wake page"
            );
            let generation = item["assignment_generation"]
                .as_i64()
                .filter(|n| *n > 0)
                .context("invalid generation")?;
            let attempt = oauth::random()?;
            tx.execute("INSERT OR IGNORE INTO v2_wake_attempts(assistant_id,delivery_id,session_id,registration_revision,assignment_generation,attempt_id,status) VALUES(?,?,?,?,?,?,'pending')",params![x.assistant_id,delivery,x.session_id,revision,generation,attempt])?;
            tx.execute("INSERT OR IGNORE INTO v2_references(assistant_id,delivery_id,session_id,assignment_generation,message_id) VALUES(?,?,?,?,?)",params![x.assistant_id,delivery,x.session_id,generation,message])?;
        }
        tx.commit()?;
        let next = v["next_cursor"].as_str().map(str::to_owned);
        ensure!(
            next.is_none() || next != cursor,
            "wake cursor did not advance"
        );
        cursor = next;
        if cursor.is_none() {
            break;
        }
    }
    Ok(())
}
fn dispatch(db: &Connection, client: &Client, c: &Config, x: &Session) -> Result<()> {
    let binding: String = db.query_row(
        "SELECT binding_id FROM v2_pairs WHERE session_id=?",
        [&x.session_id],
        |r| r.get(0),
    )?;
    let mut stmt=db.prepare("SELECT w.delivery_id,r.message_id,w.assignment_generation,w.attempt_id FROM v2_wake_attempts w JOIN v2_references r ON r.assistant_id=w.assistant_id AND r.delivery_id=w.delivery_id AND r.session_id=w.session_id AND r.assignment_generation=w.assignment_generation WHERE w.assistant_id=? AND w.session_id=? AND w.status='pending' ORDER BY w.rowid")?;
    let rows = stmt
        .query_map(params![x.assistant_id, x.session_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (delivery, message, generation, attempt) in rows {
        call(
            client,
            c,
            "works_wake_check",
            json!({"assistant_id":x.assistant_id,"binding_id":binding,"delivery_id":delivery,"assignment_generation":generation}),
        )?;
        let adapter = x.codex();
        let prepared = codex::prepare_v2(
            &adapter,
            &x.assistant_id,
            &x.session_id,
            &delivery,
            &message,
            generation,
            &attempt,
        )?;
        db.execute(
            "UPDATE v2_wake_attempts SET status='sending' WHERE attempt_id=? AND status='pending'",
            [&attempt],
        )?;
        let (status, queue) = match prepared.send() {
            AdapterOutcome::Accepted(q) => ("accepted", q),
            AdapterOutcome::Unknown => ("unknown", None),
            AdapterOutcome::NotAttempted => ("pending", None),
        };
        db.execute("UPDATE v2_wake_attempts SET status=?,queue_id=? WHERE attempt_id=? AND status='sending'",params![status,queue,attempt])?;
        ensure!(
            status != "pending",
            "Codex could not start; wake not attempted"
        );
    }
    Ok(())
}
pub fn main(args: &[String]) -> Result<()> {
    let (s, c) = settings(Path::new(&args[2]))?;
    if args[3] == "login" {
        ensure!(args.len() == 4, "login takes no arguments");
        return oauth::login(&c);
    }
    let (mut db, _lock) = db(&c)?;
    v2_journal::initialize(&db)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS v2_pairs(session_id TEXT PRIMARY KEY,assistant_id TEXT NOT NULL,binding_id TEXT NOT NULL,target TEXT NOT NULL,registration_revision INTEGER NOT NULL,status TEXT NOT NULL); CREATE TABLE IF NOT EXISTS v2_pair_history(session_id TEXT NOT NULL,assistant_id TEXT NOT NULL,binding_id TEXT NOT NULL,target TEXT NOT NULL,registration_revision INTEGER NOT NULL,status TEXT NOT NULL); CREATE TABLE IF NOT EXISTS v2_references(assistant_id TEXT NOT NULL,delivery_id TEXT NOT NULL,session_id TEXT NOT NULL,assignment_generation INTEGER NOT NULL,message_id TEXT NOT NULL,PRIMARY KEY(assistant_id,delivery_id,session_id,assignment_generation)); UPDATE v2_pairs SET status='unknown' WHERE status='sending';")?;
    if args[3] == "import-legacy" {
        ensure!(
            args.len() == 5 && Path::new(&args[4]).is_absolute(),
            "import-legacy requires an absolute legacy state directory"
        );
        println!(
            "quarantined {} legacy references",
            v2_journal::import_file(&mut db, Path::new(&args[4]))?
        );
        return Ok(());
    }
    if args[3] == "status" {
        if args.len() == 5 && args[4] == "--items" {
            let mut stmt=db.prepare("SELECT attempt_id,session_id,status FROM v2_wake_attempts WHERE status IN ('pending','unknown','blocked') ORDER BY rowid")?;
            for row in stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })? {
                let (attempt, session, status) = row?;
                println!("{status} {session} {attempt}");
            }
            return Ok(());
        }
        ensure!(args.len() == 4, "status accepts only --items");
        return v2_journal::status(&db);
    }
    if args[3] == "reconcile" {
        ensure!(
            args.len() == 6,
            "reconcile requires ATTEMPT_ID accepted|blocked"
        );
        return v2_journal::resolve(&db, &args[4], &args[5]);
    }
    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(30))
        .build()?;
    identity(&db, &client, &c)?;
    if args[3] == "pair-renew" {
        ensure!(args.len() == 5, "pair-renew requires SESSION_ID");
        let x = s
            .sessions
            .iter()
            .find(|x| x.session_id == args[4])
            .context("unknown configured session")?;
        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO v2_pair_history SELECT * FROM v2_pairs WHERE session_id=?",
            [&x.session_id],
        )?;
        tx.execute("DELETE FROM v2_pairs WHERE session_id=?", [&x.session_id])?;
        tx.commit()?;
        return pair(&db, &client, &c, x);
    }
    if args[3] == "pair" {
        ensure!(args.len() == 5, "pair requires SESSION_ID");
        let x = s
            .sessions
            .iter()
            .find(|x| x.session_id == args[4])
            .context("unknown configured session")?;
        return pair(&db, &client, &c, x);
    }
    ensure!(
        ["scan", "once", "run"].contains(&args[3].as_str()) && args.len() == 4,
        "v2 commands: login|pair SESSION_ID|pair-renew SESSION_ID|scan|once|run|status|reconcile ATTEMPT_ID accepted|blocked"
    );
    loop {
        let mut errors = 0;
        for x in &s.sessions {
            let result = scan(&mut db, &client, &c, x).and_then(|_| {
                if args[3] == "scan" {
                    Ok(())
                } else {
                    dispatch(&db, &client, &c, x)
                }
            });
            if let Err(e) = result {
                errors += 1;
                eprintln!("session {} paused: {e:#}", x.session_id);
            }
        }
        if args[3] != "run" {
            ensure!(errors == 0, "one or more sessions failed");
            break;
        }
        thread::sleep(Duration::from_secs(30));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn envelope(data: Value) -> Value {
        json!({"result":{"structuredContent":{"ok":true,"data":data}}})
    }
    #[test]
    fn polling_deduplicates_and_only_sends_after_server_check() {
        let page = envelope(
            json!({"items":[{"delivery_id":"delivery","message_id":"message","session_id":"session","assignment_generation":1}],"next_cursor":null}),
        );
        let (url, handle) =
            crate::tests::server(vec![page.clone(), page, envelope(json!({"valid":true}))]);
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let token = dir.path().join("oauth.json");
        fs::write(&token,json!({"version":1,"v2_installation_id":"bridge","origin":url,"client_id":"client","resource":format!("{url}/mcp"),"space_id":"v2","actor_id":"v2","consumer_id":"v2","access_token":"access","refresh_token":"refresh","expires_at":9999999999u64,"refresh_pending":false}).to_string()).unwrap();
        fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
        let executable = dir.path().join("codex");
        fs::write(&executable,"#!/bin/sh\necho 'Queued message queue1234 for thread 12345678-1234-1234-1234-123456789abc.'\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let session = Session {
            assistant_id: "assistant".into(),
            session_id: "session".into(),
            thread_id: "12345678-1234-1234-1234-123456789abc".into(),
            codex_path: executable,
            codex_remote: None,
            codex_home: None,
        };
        let config = Config {
            base_url: url.clone(),
            v2: true,
            token_file: token.clone(),
            oauth: Some(oauth::OAuthConfig {
                credentials_file: token,
                client_id: Some("client".into()),
                redirect_port: 0,
            }),
            require_receiver_verification: true,
            state_dir: dir.path().into(),
            space_id: "v2".into(),
            actor_id: "v2".into(),
            consumer_id: "v2".into(),
            adapter: Adapter::Codex(session.codex()),
        };
        let mut db = Connection::open_in_memory().unwrap();
        v2_journal::initialize(&db).unwrap();
        db.execute_batch("CREATE TABLE v2_pairs(session_id TEXT PRIMARY KEY,assistant_id TEXT,binding_id TEXT,target TEXT,registration_revision INTEGER,status TEXT); CREATE TABLE v2_references(assistant_id TEXT,delivery_id TEXT,session_id TEXT,assignment_generation INTEGER,message_id TEXT,PRIMARY KEY(assistant_id,delivery_id,session_id,assignment_generation));").unwrap();
        db.execute(
            "INSERT INTO v2_pairs VALUES ('session','assistant','binding',?,1,'sent')",
            [session.target(&url)],
        )
        .unwrap();
        let client = Client::builder().redirect(Policy::none()).build().unwrap();
        scan(&mut db, &client, &config, &session).unwrap();
        scan(&mut db, &client, &config, &session).unwrap();
        dispatch(&db, &client, &config, &session).unwrap();
        let status: String = db
            .query_row("SELECT status FROM v2_wake_attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "accepted");
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM v2_wake_attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let requests = handle.join().unwrap();
        assert!(requests[2].contains("works_wake_check"));
    }
}
