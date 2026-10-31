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
    #[serde(default)]
    thread_id: String,
    #[serde(default)]
    codex_path: PathBuf,
    codex_remote: Option<String>,
    codex_home: Option<PathBuf>,
    #[serde(default)]
    webhook: Option<WebhookSettings>,
    #[serde(skip)]
    pinned_webhook: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WebhookSettings {
    url_file: PathBuf,
    secret_file: PathBuf,
    #[serde(default)]
    local_loopback: bool,
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
    fn webhook(&self, token: &Path) -> Result<Option<webhook::Target>> {
        let target = self
            .webhook
            .as_ref()
            .map(|w| {
                webhook::Target::new(
                    w.url_file.clone(),
                    w.secret_file.clone(),
                    w.local_loopback,
                    &[],
                    token,
                )
            })
            .transpose()?;
        if let (Some(expected), Some(actual)) = (&self.pinned_webhook, &target) {
            ensure!(
                *expected == actual.binding(),
                "webhook target changed during this run"
            );
        }
        Ok(target)
    }
    fn target(&self, origin: &str, token: &Path) -> Result<String> {
        let webhook = self.webhook(token)?;
        Ok(self.target_for(origin, webhook.as_ref()))
    }
    fn target_for(&self, origin: &str, webhook: Option<&webhook::Target>) -> String {
        if let Some(target) = webhook {
            return format!(
                "wptarget_{}",
                hex_sha(
                    &json!([
                        "webhook-v2",
                        origin,
                        self.assistant_id,
                        self.session_id,
                        target.binding()
                    ])
                    .to_string()
                )
            );
        }
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
    let mut s: Settings = serde_json::from_slice(&fs::read(path)?)?;
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
    for x in &mut s.sessions {
        ensure!(
            id(&x.assistant_id) && id(&x.session_id),
            "invalid session configuration"
        );
        if x.webhook.is_some() {
            ensure!(
                x.thread_id.is_empty()
                    && x.codex_path.as_os_str().is_empty()
                    && x.codex_home.is_none()
                    && x.codex_remote.is_none(),
                "webhook and Codex fields cannot be mixed"
            );
            let webhook = x
                .webhook(&s.oauth.credentials_file)?
                .context("missing webhook")?;
            x.pinned_webhook = Some(webhook.binding());
        } else {
            ensure!(
                uuid.is_match(&x.thread_id)
                    && x.codex_path.is_absolute()
                    && x.codex_home.as_ref().is_none_or(|p| p.is_absolute()),
                "invalid session configuration"
            );
        }
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
    let first = &s.sessions[0];
    let adapter = match first.webhook(&s.oauth.credentials_file)? {
        Some(w) => Adapter::Webhook(w),
        None => Adapter::Codex(first.codex()),
    };
    let c = Config {
        base_url: s.base_url.clone(),
        v2: true,
        token_file: s.oauth.credentials_file.clone(),
        oauth: Some(oauth::OAuthConfig {
            credentials_file: s.oauth.credentials_file.clone(),
            client_id: s.oauth.client_id.clone(),
            redirect_port: s.oauth.redirect_port,
        }),
        #[cfg(test)]
        require_receiver_verification: true,
        state_dir: s.state_dir.clone(),
        space_id: "v2".into(),
        actor_id: "v2".into(),
        consumer_id: "v2".into(),
        adapter,
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
    if result["ok"] != true {
        return Err(McpFailure {
            tool: name.to_owned(),
            code: result["error"]["code"]
                .as_str()
                .unwrap_or("invalid response")
                .to_owned(),
        }
        .into());
    }
    Ok(result["data"].clone())
}
#[derive(Debug)]
struct McpFailure {
    tool: String,
    code: String,
}
impl std::fmt::Display for McpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MCP tool {} failed: {}", self.tool, self.code)
    }
}
impl std::error::Error for McpFailure {}
fn assignment_changed(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<McpFailure>()
        .is_some_and(|e| e.tool == "works_wake_check" && e.code == "ASSIGNMENT_CHANGED")
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
        "bridge installation changed; preserve credentials and journal; cross-installation recovery is not yet supported"
    );
    db.execute(
        "INSERT OR IGNORE INTO meta VALUES ('v2_installation',?)",
        [installation],
    )?;
    Ok(())
}
fn pair(db: &Connection, client: &Client, c: &Config, x: &Session) -> Result<()> {
    let webhook_adapter = x.webhook(&c.token_file)?;
    let target = x.target_for(&c.base_url, webhook_adapter.as_ref());
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
    let pending: Option<(String, String, String)> = db
        .query_row(
            "SELECT request_id,assistant_id,target FROM v2_pair_requests WHERE session_id=?",
            [&x.session_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let req = if let Some((req, assistant, old_target)) = pending {
        ensure!(
            assistant == x.assistant_id && old_target == target,
            "pending pairing belongs to a different target"
        );
        req
    } else {
        let req = oauth::random()?;
        db.execute("INSERT INTO v2_pair_requests(session_id,request_id,assistant_id,target) VALUES(?,?,?,?)",params![x.session_id,req,x.assistant_id,target])?;
        req
    };
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
    let codex_adapter = x.codex();
    let prepared = match &webhook_adapter {
        Some(w) => PreparedAdapter::Webhook(w.prepare_pairing_v2(
            &c.token_file,
            webhook::Reference {
                origin: &c.base_url,
                space: &x.assistant_id,
                actor: &x.session_id,
                consumer: "v2",
                item: binding,
                message: challenge,
            },
            &target,
        )?),
        None => PreparedAdapter::Codex(codex::prepare_pairing_v2(
            &codex_adapter,
            &x.assistant_id,
            &x.session_id,
            binding,
            challenge,
        )?),
    };
    db.execute("INSERT INTO v2_pairs(session_id,assistant_id,binding_id,target,registration_revision,status) VALUES(?,?,?,?,?,'sending')",params![x.session_id,x.assistant_id,binding,target,revision])?;
    db.execute(
        "INSERT INTO v2_pair_request_history SELECT * FROM v2_pair_requests WHERE session_id=?",
        [&x.session_id],
    )?;
    db.execute(
        "DELETE FROM v2_pair_requests WHERE session_id=?",
        [&x.session_id],
    )?;
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
        target == x.target(&c.base_url, &c.token_file)?,
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
    let (binding, target): (String, String) = db.query_row(
        "SELECT binding_id,target FROM v2_pairs WHERE session_id=? AND assistant_id=?",
        params![x.session_id, x.assistant_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
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
        let check = call(
            client,
            c,
            "works_wake_check",
            json!({"assistant_id":x.assistant_id,"binding_id":binding,"delivery_id":delivery,"assignment_generation":generation}),
        );
        if let Err(error) = check {
            if assignment_changed(&error) {
                ensure!(db.execute("UPDATE v2_wake_attempts SET status='blocked' WHERE attempt_id=? AND status='pending'",[&attempt])? == 1, "stale wake attempt changed");
                continue;
            }
            return Err(error);
        }
        let webhook_adapter = x.webhook(&c.token_file)?;
        ensure!(
            target == x.target_for(&c.base_url, webhook_adapter.as_ref()),
            "session adapter differs from pairing"
        );
        let adapter = x.codex();
        let prepared = match &webhook_adapter {
            Some(w) => PreparedAdapter::Webhook(w.prepare_v2(
                &c.token_file,
                webhook::Reference {
                    origin: &c.base_url,
                    space: &x.assistant_id,
                    actor: &x.session_id,
                    consumer: "v2",
                    item: &delivery,
                    message: &message,
                },
                generation,
                &attempt,
            )?),
            None => PreparedAdapter::Codex(codex::prepare_v2(
                &adapter,
                &x.assistant_id,
                &x.session_id,
                &delivery,
                &message,
                generation,
                &attempt,
            )?),
        };
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
fn offline_status_db(path: &Path) -> Result<(Connection, fs::File)> {
    private_file(path)?;
    let s: Settings = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        s.config_version == 3
            && s.state_dir.is_absolute()
            && s.oauth.credentials_file.is_absolute(),
        "invalid v2 status configuration"
    );
    let origin = url::Url::parse(&s.base_url)?;
    let loopback = origin.host_str().is_some_and(|h| {
        h == "localhost" || h.parse::<std::net::IpAddr>().is_ok_and(|a| a.is_loopback())
    });
    ensure!(
        origin.username().is_empty()
            && origin.password().is_none()
            && origin.query().is_none()
            && origin.fragment().is_none()
            && (origin.path() == "/" || origin.path().is_empty())
            && (origin.scheme() == "https" || origin.scheme() == "http" && loopback),
        "invalid service origin"
    );
    let metadata = fs::symlink_metadata(&s.state_dir)?;
    ensure!(
        metadata.is_dir(),
        "state directory must be a directory, not a symlink"
    );
    #[cfg(unix)]
    ensure!(
        metadata.uid() == unsafe_uid() && metadata.mode() & 0o077 == 0,
        "state directory must be owned by the current user and private"
    );
    let lock_path = s.state_dir.join("lock");
    private_file(&lock_path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let lock = options.open(lock_path)?;
    lock.try_lock_exclusive()
        .context("another bridge owns this state directory")?;
    let db_path = s.state_dir.join("state.sqlite3");
    private_file(&db_path)?;
    let db = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let binding: String = db
        .query_row("SELECT value FROM meta WHERE key='binding'", [], |r| {
            r.get(0)
        })
        .context("missing journal binding")?;
    let bound: Value = serde_json::from_str(&binding)?;
    let fields = bound.as_array().context("invalid journal binding")?;
    let offset = usize::from(fields.first().and_then(Value::as_str) == Some("webhook-v1"));
    ensure!(
        fields.get(offset).and_then(Value::as_str) == Some(s.base_url.trim_end_matches('/'))
            && fields.get(offset + 1).and_then(Value::as_str) == Some("v2")
            && fields.get(offset + 2).and_then(Value::as_str) == Some("v2")
            && fields.get(offset + 3).and_then(Value::as_str) == Some("v2"),
        "state directory belongs to a different service or protocol"
    );
    let installation: String = db
        .query_row(
            "SELECT value FROM meta WHERE key='v2_installation'",
            [],
            |r| r.get(0),
        )
        .context("journal has no pinned installation")?;
    ensure!(id(&installation), "invalid pinned installation");
    let credential_present = match fs::symlink_metadata(&s.oauth.credentials_file) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };
    if credential_present {
        private_file(&s.oauth.credentials_file)?;
        let mut bytes = Vec::new();
        fs::File::open(&s.oauth.credentials_file)?
            .take(16385)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 16384, "OAuth credential file too large");
        let credential: Value =
            serde_json::from_slice(&bytes).context("invalid OAuth credential file")?;
        let expected_resource = format!("{}/mcp", s.base_url.trim_end_matches('/'));
        ensure!(
            credential["version"] == 1
                && credential["v2_installation_id"].as_str() == Some(&installation)
                && credential["origin"].as_str() == Some(s.base_url.trim_end_matches('/'))
                && credential["resource"].as_str() == Some(expected_resource.as_str()),
            "OAuth credential does not match the journal"
        );
    }
    Ok((db, lock))
}
pub fn main(args: &[String]) -> Result<()> {
    if args[3] == "status" {
        ensure!(
            args.len() == 4 || args.len() == 5 && args[4] == "--items",
            "status accepts only --items"
        );
        let (db, _lock) = offline_status_db(Path::new(&args[2]))?;
        if args.len() == 5 {
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
        return v2_journal::status(&db);
    }
    let (s, c) = settings(Path::new(&args[2]))?;
    if args[3] == "login" {
        ensure!(args.len() == 4, "login takes no arguments");
        return oauth::login(&c);
    }
    let (mut db, _lock) = db(&c)?;
    v2_journal::initialize(&db)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS v2_pairs(session_id TEXT PRIMARY KEY,assistant_id TEXT NOT NULL,binding_id TEXT NOT NULL,target TEXT NOT NULL,registration_revision INTEGER NOT NULL,status TEXT NOT NULL); CREATE TABLE IF NOT EXISTS v2_pair_requests(session_id TEXT PRIMARY KEY,request_id TEXT NOT NULL,assistant_id TEXT NOT NULL,target TEXT NOT NULL); CREATE TABLE IF NOT EXISTS v2_pair_request_history(session_id TEXT NOT NULL,request_id TEXT NOT NULL,assistant_id TEXT NOT NULL,target TEXT NOT NULL); CREATE TABLE IF NOT EXISTS v2_pair_history(session_id TEXT NOT NULL,assistant_id TEXT NOT NULL,binding_id TEXT NOT NULL,target TEXT NOT NULL,registration_revision INTEGER NOT NULL,status TEXT NOT NULL); CREATE TABLE IF NOT EXISTS v2_references(assistant_id TEXT NOT NULL,delivery_id TEXT NOT NULL,session_id TEXT NOT NULL,assignment_generation INTEGER NOT NULL,message_id TEXT NOT NULL,PRIMARY KEY(assistant_id,delivery_id,session_id,assignment_generation)); UPDATE v2_pairs SET status='unknown' WHERE status='sending';")?;
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
    if args[3] == "mount-request" {
        ensure!(args.len() == 5, "mount-request requires ASSISTANT_ID");
        ensure!(
            s.sessions.iter().any(|x| x.assistant_id == args[4]),
            "assistant is not configured"
        );
        let result = call(
            &client,
            &c,
            "works_mount_request",
            json!({"assistant_id":args[4]}),
        )?;
        let request = result["request_id"]
            .as_str()
            .filter(|x| id(x))
            .context("missing mount request")?;
        println!(
            "Request {request}; review at {}/dashboard?mount_request={request}",
            c.base_url.trim_end_matches('/')
        );
        return Ok(());
    }
    if args[3] == "mount-status" {
        ensure!(
            args.len() == 5 && id(&args[4]),
            "mount-status requires REQUEST_ID"
        );
        let result = call(
            &client,
            &c,
            "works_mount_status",
            json!({"request_id":args[4]}),
        )?;
        println!("{}", result);
        return Ok(());
    }
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
        tx.execute(
            "INSERT INTO v2_pair_request_history SELECT * FROM v2_pair_requests WHERE session_id=?",
            [&x.session_id],
        )?;
        tx.execute(
            "DELETE FROM v2_pair_requests WHERE session_id=?",
            [&x.session_id],
        )?;
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
        "v2 commands: login|mount-request ASSISTANT_ID|mount-status REQUEST_ID|pair SESSION_ID|pair-renew SESSION_ID|scan|once|run|status|reconcile ATTEMPT_ID accepted|blocked"
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
    fn only_explicit_wake_check_assignment_change_is_skipped() {
        let failure = |tool: &str, code: &str| {
            anyhow::Error::new(McpFailure {
                tool: tool.into(),
                code: code.into(),
            })
        };
        assert!(assignment_changed(&failure(
            "works_wake_check",
            "ASSIGNMENT_CHANGED"
        )));
        assert!(!assignment_changed(&failure(
            "works_wake_check",
            "UNAUTHENTICATED"
        )));
        assert!(!assignment_changed(&failure(
            "works_wake_poll",
            "ASSIGNMENT_CHANGED"
        )));
        assert!(!assignment_changed(&anyhow::anyhow!(
            "network error ASSIGNMENT_CHANGED"
        )));
    }
    #[test]
    fn changed_webhook_after_scan_is_never_sent_to_new_target() {
        use std::net::TcpListener;
        let a = TcpListener::bind("127.0.0.1:0").unwrap();
        let b = TcpListener::bind("127.0.0.1:0").unwrap();
        a.set_nonblocking(true).unwrap();
        b.set_nonblocking(true).unwrap();
        let page = envelope(
            json!({"items":[{"delivery_id":"delivery","message_id":"message","session_id":"session","assignment_generation":1}],"next_cursor":null}),
        );
        let (origin, upstream) = crate::tests::server(vec![page, envelope(json!({"valid":true}))]);
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let url = dir.path().join("url");
        let secret = dir.path().join("secret");
        let token = dir.path().join("oauth");
        for (p,v) in [(&url,format!("http://{}/wake",a.local_addr().unwrap())),(&secret,"separate-secret".into()),(&token,json!({"version":1,"v2_installation_id":"bridge","origin":origin,"client_id":"client","resource":format!("{origin}/mcp"),"space_id":"v2","actor_id":"v2","consumer_id":"v2","access_token":"access","refresh_token":"refresh","expires_at":9999999999u64,"refresh_pending":false}).to_string())] {fs::write(p,v).unwrap();fs::set_permissions(p,fs::Permissions::from_mode(0o600)).unwrap();}
        let session = Session {
            assistant_id: "assistant".into(),
            session_id: "session".into(),
            thread_id: String::new(),
            codex_path: PathBuf::new(),
            codex_remote: None,
            codex_home: None,
            pinned_webhook: None,
            webhook: Some(WebhookSettings {
                url_file: url.clone(),
                secret_file: secret,
                local_loopback: true,
            }),
        };
        let target = session.webhook(&token).unwrap().unwrap();
        let c = Config {
            base_url: origin.clone(),
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
            adapter: Adapter::Webhook(target),
        };
        let mut db = Connection::open_in_memory().unwrap();
        v2_journal::initialize(&db).unwrap();
        db.execute_batch("CREATE TABLE v2_pairs(session_id TEXT PRIMARY KEY,assistant_id TEXT,binding_id TEXT,target TEXT,registration_revision INTEGER,status TEXT); CREATE TABLE v2_references(assistant_id TEXT,delivery_id TEXT,session_id TEXT,assignment_generation INTEGER,message_id TEXT,PRIMARY KEY(assistant_id,delivery_id,session_id,assignment_generation));").unwrap();
        db.execute(
            "INSERT INTO v2_pairs VALUES ('session','assistant','binding',?,1,'sent')",
            [session.target(&origin, &c.token_file).unwrap()],
        )
        .unwrap();
        let client = Client::builder().redirect(Policy::none()).build().unwrap();
        scan(&mut db, &client, &c, &session).unwrap();
        fs::write(&url, format!("http://{}/wake", b.local_addr().unwrap())).unwrap();
        assert!(dispatch(&db, &client, &c, &session).is_err());
        assert!(a.accept().is_err());
        assert!(b.accept().is_err());
        let state: String = db
            .query_row("SELECT status FROM v2_wake_attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(state, "pending");
        upstream.join().unwrap();
    }
    #[test]
    fn stale_assignment_is_blocked_and_next_assignment_is_woken() {
        let invalid = json!({"result":{"structuredContent":{"ok":false,"error":{"code":"ASSIGNMENT_CHANGED"}}}});
        let (origin, upstream) =
            crate::tests::server(vec![invalid, envelope(json!({"valid":true}))]);
        let (dir, target, token, receiver) = crate::webhook::tests::fixture("202 Accepted");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&token,json!({"version":1,"v2_installation_id":"bridge","origin":origin,"client_id":"client","resource":format!("{origin}/mcp"),"space_id":"v2","actor_id":"v2","consumer_id":"v2","access_token":"access","refresh_token":"refresh","expires_at":9999999999u64,"refresh_pending":false}).to_string()).unwrap();
        let session = Session {
            assistant_id: "assistant".into(),
            session_id: "session".into(),
            thread_id: String::new(),
            codex_path: PathBuf::new(),
            codex_remote: None,
            codex_home: None,
            pinned_webhook: None,
            webhook: Some(WebhookSettings {
                url_file: dir.path().join("url"),
                secret_file: dir.path().join("secret"),
                local_loopback: true,
            }),
        };
        let c = Config {
            base_url: origin.clone(),
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
            adapter: Adapter::Webhook(target),
        };
        let db = Connection::open_in_memory().unwrap();
        v2_journal::initialize(&db).unwrap();
        db.execute_batch("CREATE TABLE v2_pairs(session_id TEXT PRIMARY KEY,assistant_id TEXT,binding_id TEXT,target TEXT,registration_revision INTEGER,status TEXT); CREATE TABLE v2_references(assistant_id TEXT,delivery_id TEXT,session_id TEXT,assignment_generation INTEGER,message_id TEXT,PRIMARY KEY(assistant_id,delivery_id,session_id,assignment_generation)); INSERT INTO v2_wake_attempts VALUES('assistant','old','session',1,1,'attempt_old','pending',NULL),('assistant','new','session',1,1,'attempt_new','pending',NULL); INSERT INTO v2_references VALUES('assistant','old','session',1,'message_old'),('assistant','new','session',1,'message_new');").unwrap();
        db.execute(
            "INSERT INTO v2_pairs VALUES ('session','assistant','binding',?,1,'sent')",
            [session.target(&origin, &c.token_file).unwrap()],
        )
        .unwrap();
        let client = Client::builder().redirect(Policy::none()).build().unwrap();
        dispatch(&db, &client, &c, &session).unwrap();
        let old: String = db
            .query_row(
                "SELECT status FROM v2_wake_attempts WHERE delivery_id='old'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let new: String = db
            .query_row(
                "SELECT status FROM v2_wake_attempts WHERE delivery_id='new'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!((old, new), ("blocked".into(), "accepted".into()));
        let (_, body) = receiver.join().unwrap();
        assert_eq!(body["delivery_id"], "new");
        upstream.join().unwrap();
    }
    #[test]
    fn existing_codex_target_fingerprint_is_unchanged() {
        let x = Session {
            assistant_id: "assistant".into(),
            session_id: "session".into(),
            thread_id: "12345678-1234-1234-1234-123456789abc".into(),
            codex_path: "/usr/local/bin/codex".into(),
            codex_remote: None,
            codex_home: None,
            pinned_webhook: None,
            webhook: None,
        };
        assert_eq!(
            x.target("https://example.com", Path::new("/unused"))
                .unwrap(),
            "wptarget_a9b68fe38aaaa6c00e61fe3eb7dc8baffae9abf1c8028cc427d92fad7fc2ebc4"
        );
    }
    #[test]
    fn pairing_reuses_persisted_request_after_failed_response() {
        let (url, upstream) = crate::tests::server(vec![
            json!({"error":{"code":"INVALID"}}),
            envelope(
                json!({"binding_id":"binding","challenge":"challenge","registration_revision":1}),
            ),
        ]);
        let (dir, target, token, receiver) = crate::webhook::tests::fixture("202 Accepted");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&token,json!({"version":1,"v2_installation_id":"bridge","origin":url,"client_id":"client","resource":format!("{url}/mcp"),"space_id":"v2","actor_id":"v2","consumer_id":"v2","access_token":"access","refresh_token":"refresh","expires_at":9999999999u64,"refresh_pending":false}).to_string()).unwrap();
        let session = Session {
            assistant_id: "assistant".into(),
            session_id: "session".into(),
            thread_id: String::new(),
            codex_path: PathBuf::new(),
            codex_remote: None,
            codex_home: None,
            pinned_webhook: None,
            webhook: Some(WebhookSettings {
                url_file: dir.path().join("url"),
                secret_file: dir.path().join("secret"),
                local_loopback: true,
            }),
        };
        let c = Config {
            base_url: url,
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
            adapter: Adapter::Webhook(target),
        };
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE v2_pairs(session_id TEXT PRIMARY KEY,assistant_id TEXT,binding_id TEXT,target TEXT,registration_revision INTEGER,status TEXT); CREATE TABLE v2_pair_requests(session_id TEXT PRIMARY KEY,request_id TEXT NOT NULL,assistant_id TEXT NOT NULL,target TEXT NOT NULL); CREATE TABLE v2_pair_request_history(session_id TEXT NOT NULL,request_id TEXT NOT NULL,assistant_id TEXT NOT NULL,target TEXT NOT NULL);").unwrap();
        let client = Client::builder().redirect(Policy::none()).build().unwrap();
        assert!(pair(&db, &client, &c, &session).is_err());
        let request_id: String = db
            .query_row("SELECT request_id FROM v2_pair_requests", [], |r| r.get(0))
            .unwrap();
        pair(&db, &client, &c, &session).unwrap();
        let (_, body) = receiver.join().unwrap();
        assert_eq!(body["event"], "miles-works.wake.challenge");
        let requests = upstream.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.contains(&request_id)));
        let status: String = db
            .query_row("SELECT status FROM v2_pairs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "sent");
        let history: String = db
            .query_row("SELECT request_id FROM v2_pair_request_history", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(history, request_id);
    }
    #[test]
    fn webhook_configuration_requires_explicit_literal_loopback_and_no_codex_fields() {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let url = dir.path().join("url");
        let secret = dir.path().join("secret");
        for (path, value) in [
            (&url, "http://127.0.0.1:9876/wake"),
            (&secret, "independent-secret"),
        ] {
            fs::write(path, value).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let path = dir.path().join("config.json");
        let mut v = json!({"config_version":3,"base_url":"https://example.com","state_dir":dir.path().join("state"),"oauth":{"credentials_file":dir.path().join("oauth.json"),"client_id":null,"redirect_port":0},"sessions":[{"assistant_id":"assistant","session_id":"session","webhook":{"url_file":url,"secret_file":secret,"local_loopback":true}}]});
        let write = |value: &Value| fs::write(&path, value.to_string()).unwrap();
        write(&v);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let (settings_snapshot, c) = settings(&path).unwrap();
        fs::write(&url, "http://127.0.0.1:9877/wake").unwrap();
        assert!(
            settings_snapshot.sessions[0]
                .webhook(&c.token_file)
                .is_err()
        );
        fs::write(&url, "http://127.0.0.1:9876/wake").unwrap();
        {
            let (db, _lock) = db(&c).unwrap();
            v2_journal::initialize(&db).unwrap();
            db.execute(
                "INSERT INTO meta VALUES('v2_installation','installation')",
                [],
            )
            .unwrap();
            db.execute("INSERT INTO v2_wake_attempts VALUES('assistant','delivery','session',1,1,'attempt','unknown',NULL)",[]).unwrap();
        }
        fs::remove_file(&secret).unwrap();
        let args = vec![
            "bridge".into(),
            "--config".into(),
            path.to_str().unwrap().into(),
            "status".into(),
            "--items".into(),
        ];
        main(&args).unwrap();
        let (db, _lock) = offline_status_db(&path).unwrap();
        let status: String = db
            .query_row("SELECT status FROM v2_wake_attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "unknown");
        drop(db);
        drop(_lock);
        let oauth = dir.path().join("oauth.json");
        fs::write(&oauth,json!({"version":1,"v2_installation_id":"different","origin":"https://example.com","resource":"https://example.com/mcp"}).to_string()).unwrap();
        fs::set_permissions(&oauth, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(offline_status_db(&path).is_err());
        fs::remove_file(&oauth).unwrap();
        std::os::unix::fs::symlink(dir.path().join("missing-credential"), &oauth).unwrap();
        assert!(offline_status_db(&path).is_err());
        fs::remove_file(oauth).unwrap();
        fs::write(&secret, "independent-secret").unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
        v["sessions"][0]["webhook"]["local_loopback"] = json!(false);
        write(&v);
        assert!(settings(&path).is_err());
        v["sessions"][0]["webhook"]["local_loopback"] = json!(true);
        v["sessions"][0]["thread_id"] = json!("12345678-1234-1234-1234-123456789abc");
        write(&v);
        assert!(settings(&path).is_err());
        v["sessions"][0]
            .as_object_mut()
            .unwrap()
            .remove("thread_id");
        fs::write(&url, "http://localhost:9876/wake").unwrap();
        write(&v);
        assert!(settings(&path).is_err());
    }
    #[test]
    fn webhook_wakes_are_deduplicated_and_unknown_is_not_replayed() {
        for (response, expected) in [
            ("202 Accepted", "accepted"),
            ("500 Internal Server Error", "unknown"),
        ] {
            let page = envelope(
                json!({"items":[{"delivery_id":"delivery","message_id":"message","session_id":"session","assignment_generation":1}],"next_cursor":null}),
            );
            let (url, upstream) =
                crate::tests::server(vec![page.clone(), page, envelope(json!({"valid":true}))]);
            let (dir, target, token, receiver) = crate::webhook::tests::fixture(response);
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(&token,json!({"version":1,"v2_installation_id":"bridge","origin":url,"client_id":"client","resource":format!("{url}/mcp"),"space_id":"v2","actor_id":"v2","consumer_id":"v2","access_token":"access","refresh_token":"refresh","expires_at":9999999999u64,"refresh_pending":false}).to_string()).unwrap();
            let session = Session {
                assistant_id: "assistant".into(),
                session_id: "session".into(),
                thread_id: String::new(),
                codex_path: PathBuf::new(),
                codex_remote: None,
                codex_home: None,
                pinned_webhook: None,
                webhook: Some(WebhookSettings {
                    url_file: dir.path().join("url"),
                    secret_file: dir.path().join("secret"),
                    local_loopback: true,
                }),
            };
            let c = Config {
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
                adapter: Adapter::Webhook(target),
            };
            let mut db = Connection::open_in_memory().unwrap();
            v2_journal::initialize(&db).unwrap();
            db.execute_batch("CREATE TABLE v2_pairs(session_id TEXT PRIMARY KEY,assistant_id TEXT,binding_id TEXT,target TEXT,registration_revision INTEGER,status TEXT); CREATE TABLE v2_references(assistant_id TEXT,delivery_id TEXT,session_id TEXT,assignment_generation INTEGER,message_id TEXT,PRIMARY KEY(assistant_id,delivery_id,session_id,assignment_generation));").unwrap();
            db.execute(
                "INSERT INTO v2_pairs VALUES ('session','assistant','binding',?,1,'sent')",
                [session.target(&url, &c.token_file).unwrap()],
            )
            .unwrap();
            let client = Client::builder().redirect(Policy::none()).build().unwrap();
            scan(&mut db, &client, &c, &session).unwrap();
            scan(&mut db, &client, &c, &session).unwrap();
            dispatch(&db, &client, &c, &session).unwrap();
            dispatch(&db, &client, &c, &session).unwrap();
            let (count, state): (i64, String) = db
                .query_row("SELECT count(*),status FROM v2_wake_attempts", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap();
            assert_eq!((count, state), (1, expected.into()));
            let (_, body) = receiver.join().unwrap();
            assert_eq!(body["event"], "miles-works.message.available");
            let requests = upstream.join().unwrap();
            assert_eq!(requests.len(), 3);
            assert!(requests[2].contains("works_wake_check"));
            assert!(
                requests
                    .iter()
                    .all(|r| !r.contains("works_deliveries_complete"))
            );
        }
    }
    #[test]
    fn changed_installation_cannot_modify_or_replay_existing_journal() {
        let (url, handle) = crate::tests::server(vec![envelope(
            json!({"installation_id":"new_installation"}),
        )]);
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let token = dir.path().join("oauth.json");
        fs::write(&token, json!({"version":1,"v2_installation_id":"new_installation","origin":url,"client_id":"client","resource":format!("{url}/mcp"),"space_id":"v2","actor_id":"v2","consumer_id":"v2","access_token":"test","refresh_token":"test","expires_at":9999999999u64,"refresh_pending":false}).to_string()).unwrap();
        fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
        let c = Config {
            base_url: url,
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
            adapter: Adapter::Codex(codex::CodexConfig {
                thread_id: "12345678-1234-1234-1234-123456789abc".into(),
                codex_path: "/tmp/codex".into(),
                codex_remote: None,
                codex_home: None,
            }),
        };
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); INSERT INTO meta VALUES ('v2_installation','old_installation');").unwrap();
        v2_journal::initialize(&db).unwrap();
        db.execute_batch("INSERT INTO v2_wake_attempts VALUES ('a','d1','s',1,1,'x','accepted','queue'); INSERT INTO v2_wake_attempts VALUES ('a','d2','s',1,1,'y','unknown',NULL);").unwrap();
        let error = identity(&db, &Client::new(), &c).unwrap_err().to_string();
        assert!(error.contains("bridge installation changed"));
        let old: String = db
            .query_row(
                "SELECT value FROM meta WHERE key='v2_installation'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old, "old_installation");
        let states: Vec<String> = db
            .prepare("SELECT status FROM v2_wake_attempts ORDER BY delivery_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(states, ["accepted", "unknown"]);
        assert_eq!(handle.join().unwrap().len(), 1);
    }
    #[test]
    fn mount_request_and_status_use_bridge_credential() {
        let (url, handle) = crate::tests::server(vec![
            envelope(json!({"installation_id":"bridge"})),
            envelope(json!({"request_id":"mount_1"})),
            envelope(json!({"installation_id":"bridge"})),
            envelope(json!({"status":"approved"})),
        ]);
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let token = dir.path().join("oauth.json");
        fs::write(&token, json!({"version":1,"v2_installation_id":"bridge","origin":url,"client_id":"client","resource":format!("{url}/mcp"),"space_id":"v2","actor_id":"v2","consumer_id":"v2","access_token":"bridge-access","refresh_token":"refresh","expires_at":9999999999u64,"refresh_pending":false}).to_string()).unwrap();
        fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
        let config = dir.path().join("v3.json");
        fs::write(&config, json!({"config_version":3,"base_url":url,"state_dir":dir.path().join("state"),"oauth":{"credentials_file":token,"client_id":"client","redirect_port":0},"sessions":[{"assistant_id":"assistant","session_id":"session","thread_id":"12345678-1234-1234-1234-123456789abc","codex_path":"/tmp/codex","codex_remote":null,"codex_home":null}]}).to_string()).unwrap();
        let p = config.to_str().unwrap();
        let args = |command: &str, value: &str| {
            vec![
                "bridge".into(),
                "--config".into(),
                p.into(),
                command.into(),
                value.into(),
            ]
        };
        main(&args("mount-request", "assistant")).unwrap();
        main(&args("mount-status", "mount_1")).unwrap();
        let requests = handle.join().unwrap();
        assert!(requests[1].contains("works_mount_request"));
        assert!(requests[1].contains("Bearer bridge-access"));
        assert!(requests[3].contains("works_mount_status"));
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
            pinned_webhook: None,
            webhook: None,
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
            [session.target(&url, &config.token_file).unwrap()],
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
