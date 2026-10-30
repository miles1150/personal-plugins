//! An independent public OAuth client. Never reads a host's MCP credentials.
use super::{Config, private_file, unsafe_uid};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use fs2::FileExt;
use reqwest::{blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthConfig {
    pub credentials_file: PathBuf,
    /// Omit to register a separate native public client on each login.
    pub client_id: Option<String>,
    #[serde(default)]
    pub redirect_port: u16,
}
impl OAuthConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.credentials_file.is_absolute(),
            "OAuth credentials_file must be absolute"
        );
        private_parent(&self.credentials_file)?;
        if self.credentials_file.exists() {
            private_file(&self.credentials_file)?;
        }
        if let Some(client_id) = &self.client_id {
            ensure!(
                !client_id.is_empty()
                    && client_id.len() <= 2048
                    && !client_id.chars().any(char::is_control),
                "invalid OAuth client_id"
            );
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    v2_installation_id: Option<String>,
    origin: String,
    client_id: String,
    resource: String,
    space_id: String,
    actor_id: String,
    consumer_id: String,
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    /// Written and fsynced before rotation; never replay this refresh token.
    refresh_pending: bool,
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn origin(c: &Config) -> &str {
    c.base_url.trim_end_matches('/')
}
fn resource(c: &Config) -> String {
    format!("{}/mcp", origin(c))
}
fn private_parent(path: &Path) -> Result<&Path> {
    let parent = path.parent().context("OAuth credential parent missing")?;
    let m = fs::symlink_metadata(parent)?;
    ensure!(
        m.is_dir(),
        "OAuth credential parent must be a directory, not a symlink"
    );
    #[cfg(unix)]
    ensure!(
        m.uid() == unsafe_uid() && m.mode() & 0o077 == 0,
        "OAuth credential parent must be owned by the current user and private (0700)"
    );
    Ok(parent)
}
fn lock(path: &Path) -> Result<File> {
    private_parent(path)?;
    let lock_path = path.with_file_name(format!(
        "{}.lock",
        path.file_name()
            .context("invalid credential filename")?
            .to_string_lossy()
    ));
    let mut opts = OpenOptions::new();
    opts.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    let file = opts.open(&lock_path)?;
    private_file(&lock_path)?;
    file.try_lock_exclusive()
        .context("another process owns the OAuth credential lock; retry later")?;
    Ok(file)
}
fn read(path: &Path) -> Result<Credentials> {
    private_file(path)?;
    let mut bytes = Vec::new();
    File::open(path)?.take(16385).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16384, "OAuth credential file too large");
    serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("invalid OAuth credential file; login required"))
}
fn save(path: &Path, value: &Credentials) -> Result<()> {
    let parent = private_parent(path)?;
    if fs::symlink_metadata(path).is_ok() {
        private_file(path)?;
    }
    let temp = parent.join(format!(".waypost-oauth-{}", random()?));
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    let result = (|| -> Result<()> {
        let mut file = opts.open(&temp)?;
        file.write_all(&serde_json::to_vec(value)?)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
fn binding(c: &Config, value: &Credentials) -> Result<()> {
    ensure!(
        value.version == 1
            && (value.v2_installation_id.is_some() == c.v2)
            && value.origin == origin(c)
            && value.resource == resource(c)
            && value.space_id == c.space_id
            && value.actor_id == c.actor_id
            && value.consumer_id == c.consumer_id
            && c.oauth
                .as_ref()
                .and_then(|x| x.client_id.as_ref())
                .is_none_or(|id| id == &value.client_id),
        "OAuth credential binding does not match configuration; login required"
    );
    ensure!(
        !value.refresh_pending,
        "OAuth refresh outcome is unknown; login required (old refresh token will not be replayed)"
    );
    Ok(())
}
pub(super) fn random() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| anyhow::anyhow!("secure random generator unavailable"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent("Waypost/0.1.0 bridge OAuth")
        .redirect(Policy::none())
        .timeout(Duration::from_secs(30))
        .build()?)
}
fn response(response: reqwest::blocking::Response) -> Result<Value> {
    let status = response.status().as_u16();
    // OAuth error bodies and malformed response text are never printed.
    ensure!(
        (200..300).contains(&status),
        "OAuth request failed (HTTP {status}); login may be required"
    );
    let mut bytes = Vec::new();
    response
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("OAuth response could not be read"))?;
    ensure!(bytes.len() <= 16384, "OAuth response too large");
    serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("invalid OAuth response"))
}
fn verify_identity(client: &Client, c: &Config, access: &str) -> Result<Option<String>> {
    if c.v2 {
        let response = client.post(format!("{}/mcp", origin(c)))
            .bearer_auth(access)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"works_identity","arguments":{}}}))
            .send().map_err(|_| anyhow::anyhow!("OAuth identity check failed"))?;
        let v = self::response(response)?;
        let data = &v["result"]["structuredContent"];
        ensure!(data["ok"] == true, "OAuth v2 identity check failed");
        let id = data["data"]["installation_id"]
            .as_str()
            .context("OAuth installation missing")?;
        ensure!(
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            "invalid OAuth installation"
        );
        return Ok(Some(id.into()));
    }
    let response = client
        .get(format!("{}/v0/describe", origin(c)))
        .bearer_auth(access)
        .send()
        .map_err(|_| anyhow::anyhow!("OAuth identity check failed"))?;
    let v = self::response(response)?;
    let d = &v["data"];
    ensure!(
        d["space_id"] == c.space_id
            && d["actor_id"] == c.actor_id
            && d["consumer_id"] == c.consumer_id,
        "OAuth credential identity does not match configuration; login required"
    );
    Ok(None)
}
fn tokens(value: &Value) -> Result<(String, String, u64)> {
    let access = value["access_token"]
        .as_str()
        .context("OAuth access token missing")?;
    let refresh = value["refresh_token"]
        .as_str()
        .context("OAuth refresh token missing")?;
    let expires = value["expires_in"]
        .as_u64()
        .context("OAuth expiry missing")?;
    ensure!(
        value["token_type"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("Bearer"))
            && !access.is_empty()
            && access.len() <= 4096
            && !access.chars().any(char::is_control)
            && !refresh.is_empty()
            && refresh.len() <= 4096
            && !refresh.chars().any(char::is_control)
            && expires > 0
            && expires <= 366 * 24 * 3600,
        "invalid OAuth token response"
    );
    Ok((
        access.into(),
        refresh.into(),
        now()?
            .checked_add(expires)
            .context("invalid OAuth expiry")?,
    ))
}
pub fn access_token(client: &Client, c: &Config) -> Result<String> {
    if c.oauth.is_none() {
        private_file(&c.token_file)?;
        let mut token = String::new();
        File::open(&c.token_file)?
            .take(8193)
            .read_to_string(&mut token)?;
        ensure!(
            token.len() <= 8192 && !token.trim().is_empty(),
            "invalid credential"
        );
        return Ok(token.trim().into());
    }
    let _lock = lock(&c.token_file)?;
    let mut value = read(&c.token_file).context("OAuth credentials unavailable; run login")?;
    binding(c, &value)?;
    if value.expires_at > now()?.saturating_add(60) {
        return Ok(value.access_token);
    }
    value.refresh_pending = true;
    save(&c.token_file, &value)?;
    let res = client
        .post(format!("{}/oauth/token", origin(c)))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &value.client_id),
            ("resource", &value.resource),
            ("refresh_token", &value.refresh_token),
        ])
        .send()
        .map_err(|_| anyhow::anyhow!("OAuth refresh outcome is unknown; login required"))?;
    let (access, refresh, expires_at) = tokens(&response(res)?)?;
    ensure!(
        verify_identity(client, c, &access)? == value.v2_installation_id,
        "OAuth installation changed; login required"
    );
    value.access_token = access;
    value.refresh_token = refresh;
    value.expires_at = expires_at;
    value.refresh_pending = false;
    save(&c.token_file, &value)?;
    Ok(value.access_token)
}

pub fn login(c: &Config) -> Result<()> {
    let oauth = c
        .oauth
        .as_ref()
        .context("login requires oauth configuration; legacy token_file is unchanged")?;
    let _lock = lock(&c.token_file)?;
    let client = client()?;
    let listener = TcpListener::bind(("127.0.0.1", oauth.redirect_port))
        .context("OAuth loopback callback could not bind")?;
    listener.set_nonblocking(true)?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr()?.port()
    );
    let scope = if c.v2 { "works:client" } else { "waypost" };
    let client_id = if let Some(id) = &oauth.client_id {
        id.clone()
    } else {
        let res = client.post(format!("{}/oauth/register", origin(c))).json(&json!({
            "client_name":"Waypost bridge", "application_type":"native", "redirect_uris":[redirect],
            "token_endpoint_auth_method":"none", "grant_types":["authorization_code","refresh_token"],
            "response_types":["code"], "scope":scope
        })).send().map_err(|_| anyhow::anyhow!("OAuth client registration failed"))?;
        let v = response(res)?;
        let id = v["client_id"]
            .as_str()
            .context("OAuth registration returned no client_id")?;
        ensure!(
            !id.is_empty() && id.len() <= 2048 && !id.chars().any(char::is_control),
            "invalid registered client_id"
        );
        id.to_owned()
    };
    let verifier = random()?;
    let state = random()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut authorize = url::Url::parse(&format!("{}/oauth/authorize", origin(c)))?;
    authorize.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", &client_id),
        ("redirect_uri", &redirect),
        ("scope", scope),
        ("resource", &resource(c)),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);
    println!("Open this URL in your browser on this device to authorize the bridge:\n{authorize}");
    let deadline = Instant::now() + Duration::from_secs(300);
    let code = loop {
        ensure!(
            Instant::now() < deadline,
            "OAuth login timed out; run login to start a new request"
        );
        match listener.accept() {
            Ok((mut stream, _)) => match callback(&mut stream, &redirect, &state, origin(c)) {
                Ok(code) => {
                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nCache-Control: no-store\r\nConnection: close\r\n\r\nAuthorization received. Return to the bridge terminal to check the result.\n");
                    break code;
                }
                Err(_) => {
                    let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nCache-Control: no-store\r\nConnection: close\r\n\r\nInvalid callback. Return to the bridge terminal.\n");
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100))
            }
            Err(_) => bail!("OAuth callback listener failed"),
        }
    };
    let res = client
        .post(format!("{}/oauth/token", origin(c)))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", &client_id),
            ("resource", &resource(c)),
            ("redirect_uri", &redirect),
            ("code", &code),
            ("code_verifier", &verifier),
        ])
        .send()
        .map_err(|_| {
            anyhow::anyhow!(
                "OAuth login exchange failed; start a new login (code will not be replayed)"
            )
        })?;
    let (access_token, refresh_token, expires_at) = tokens(&response(res)?)?;
    let installation_id = verify_identity(&client, c, &access_token)?;
    save(
        &c.token_file,
        &Credentials {
            version: 1,
            v2_installation_id: installation_id,
            origin: origin(c).into(),
            client_id,
            resource: resource(c),
            space_id: c.space_id.clone(),
            actor_id: c.actor_id.clone(),
            consumer_id: c.consumer_id.clone(),
            access_token,
            refresh_token,
            expires_at,
            refresh_pending: false,
        },
    )?;
    println!("Bridge OAuth authorization saved; configured identity verified.");
    Ok(())
}
fn callback(stream: &mut TcpStream, redirect: &str, state: &str, issuer: &str) -> Result<String> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut line = String::new();
    BufReader::new(stream.take(8193)).read_line(&mut line)?;
    ensure!(line.len() <= 8192, "callback too large");
    let parts: Vec<_> = line.split_whitespace().collect();
    ensure!(
        parts.len() == 3
            && parts[0] == "GET"
            && parts[2] == "HTTP/1.1"
            && parts[1].starts_with("/callback?"),
        "invalid callback"
    );
    let mut url = url::Url::parse(redirect)?;
    url.set_query(parts[1].split_once('?').map(|(_, q)| q));
    validate_callback(&url, state, issuer)
}
fn validate_callback(url: &url::Url, state: &str, issuer: &str) -> Result<String> {
    let mut values = std::collections::BTreeMap::new();
    for (k, v) in url.query_pairs() {
        ensure!(
            values.insert(k.into_owned(), v.into_owned()).is_none(),
            "duplicate callback parameter"
        );
    }
    ensure!(
        values.get("state").is_some_and(|s| s == state)
            && values.get("iss").is_some_and(|s| s == issuer)
            && !values.contains_key("error"),
        "callback state or issuer mismatch, or authorization denied"
    );
    let code = values
        .remove("code")
        .context("authorization code missing")?;
    ensure!(
        !code.is_empty() && code.len() <= 2048,
        "invalid authorization code"
    );
    Ok(code)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{Adapter, codex::CodexConfig};
    use std::os::unix::fs::PermissionsExt;
    fn fixture() -> (tempfile::TempDir, Config, Credentials) {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let c = Config {
            v2: false,
            base_url: "http://127.0.0.1".into(),
            token_file: dir.path().join("oauth.json"),
            oauth: Some(OAuthConfig {
                credentials_file: dir.path().join("oauth.json"),
                client_id: Some("bridge-client".into()),
                redirect_port: 0,
            }),
            require_receiver_verification: true,
            state_dir: dir.path().join("state"),
            space_id: "space".into(),
            actor_id: "actor".into(),
            consumer_id: "consumer".into(),
            adapter: Adapter::Codex(CodexConfig {
                thread_id: "12345678-1234-1234-1234-123456789abc".into(),
                codex_path: dir.path().join("codex"),
                codex_home: None,
                codex_remote: None,
            }),
        };
        let v = Credentials {
            version: 1,
            v2_installation_id: None,
            origin: origin(&c).into(),
            client_id: "bridge-client".into(),
            resource: resource(&c),
            space_id: c.space_id.clone(),
            actor_id: c.actor_id.clone(),
            consumer_id: c.consumer_id.clone(),
            access_token: "access-original".into(),
            refresh_token: "refresh-original".into(),
            expires_at: 0,
            refresh_pending: false,
        };
        (dir, c, v)
    }
    fn server(responses: Vec<Option<Value>>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut requests = vec![];
            for value in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut headers = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = n.trim().parse().unwrap();
                    }
                    headers.push_str(&line);
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                requests.push(format!("{headers}{}", String::from_utf8(body).unwrap()));
                if let Some(value) = value {
                    let body = value.to_string();
                    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                }
                // None deliberately loses the rotation response after consuming the request.
            }
            requests
        });
        (origin, handle)
    }
    fn new_tokens() -> Value {
        json!({"token_type":"Bearer","access_token":"access-rotated","refresh_token":"refresh-rotated","expires_in":3600})
    }
    #[test]
    fn rotation_saves_both_tokens_after_identity_check() {
        let (_dir, mut c, mut old) = fixture();
        let (origin, handle) = server(vec![
            Some(new_tokens()),
            Some(json!({"data":{"space_id":"space","actor_id":"actor","consumer_id":"consumer"}})),
        ]);
        c.base_url = origin;
        old.origin = super::origin(&c).into();
        old.resource = resource(&c);
        save(&c.token_file, &old).unwrap();
        let client = Client::builder().no_proxy().build().unwrap();
        assert_eq!(access_token(&client, &c).unwrap(), "access-rotated");
        let fresh = read(&c.token_file).unwrap();
        assert_eq!(fresh.refresh_token, "refresh-rotated");
        assert!(!fresh.refresh_pending);
        assert_eq!(
            fs::metadata(&c.token_file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(access_token(&client, &c).unwrap(), "access-rotated");
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].contains("refresh_token=refresh-original"));
        assert!(requests[1].contains("Bearer access-rotated"));
    }
    #[test]
    fn v2_refresh_rejects_changed_installation_without_replaying_rotation() {
        let (_dir, mut c, mut old) = fixture();
        let (origin, handle) = server(vec![
            Some(new_tokens()),
            Some(
                json!({"result":{"structuredContent":{"ok":true,"data":{"installation_id":"other"}}}}),
            ),
        ]);
        c.base_url = origin;
        c.v2 = true;
        old.origin = super::origin(&c).into();
        old.resource = resource(&c);
        old.v2_installation_id = Some("original".into());
        save(&c.token_file, &old).unwrap();
        let client = Client::builder().no_proxy().build().unwrap();
        assert!(access_token(&client, &c).is_err());
        assert!(read(&c.token_file).unwrap().refresh_pending);
        assert!(access_token(&client, &c).is_err());
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].contains("works_identity"));
    }
    #[test]
    fn lost_rotation_response_is_never_replayed() {
        let (_dir, mut c, mut old) = fixture();
        let (origin, handle) = server(vec![None]);
        c.base_url = origin;
        old.origin = super::origin(&c).into();
        old.resource = resource(&c);
        save(&c.token_file, &old).unwrap();
        let client = Client::builder().no_proxy().build().unwrap();
        assert!(access_token(&client, &c).is_err());
        assert!(read(&c.token_file).unwrap().refresh_pending);
        let error = access_token(&client, &c).unwrap_err().to_string();
        assert!(error.contains("will not be replayed"));
        assert!(!error.contains("refresh-original"));
        assert_eq!(handle.join().unwrap().len(), 1);
    }
    #[test]
    fn rotated_wrong_identity_stays_in_recovery() {
        let (_dir, mut c, mut old) = fixture();
        let (origin, handle) = server(vec![
            Some(new_tokens()),
            Some(
                json!({"data":{"space_id":"space","actor_id":"another","consumer_id":"consumer"}}),
            ),
        ]);
        c.base_url = origin;
        old.origin = super::origin(&c).into();
        old.resource = resource(&c);
        save(&c.token_file, &old).unwrap();
        let client = Client::builder().no_proxy().build().unwrap();
        assert!(
            access_token(&client, &c)
                .unwrap_err()
                .to_string()
                .contains("identity")
        );
        let saved = read(&c.token_file).unwrap();
        assert_eq!(saved.access_token, "access-original");
        assert!(saved.refresh_pending);
        handle.join().unwrap();
    }
    #[test]
    fn callback_rejects_wrong_issuer_state_and_duplicates() {
        let valid=url::Url::parse("http://127.0.0.1/callback?code=opaque&state=expected&iss=https%3A%2F%2Fwaypost.example").unwrap();
        assert_eq!(
            validate_callback(&valid, "expected", "https://waypost.example").unwrap(),
            "opaque"
        );
        assert!(validate_callback(&valid, "wrong", "https://waypost.example").is_err());
        assert!(validate_callback(&valid, "expected", "https://wrong.example").is_err());
        let mut duplicate = valid.clone();
        duplicate.query_pairs_mut().append_pair("code", "other");
        assert!(validate_callback(&duplicate, "expected", "https://waypost.example").is_err());
    }
    #[test]
    fn credential_lock_and_binding_fail_closed() {
        let (_dir, c, mut v) = fixture();
        save(&c.token_file, &v).unwrap();
        let held = lock(&c.token_file).unwrap();
        assert!(lock(&c.token_file).is_err());
        drop(held);
        assert!(lock(&c.token_file).is_ok());
        v.actor_id = "other".into();
        assert!(binding(&c, &v).is_err());
        fs::set_permissions(
            c.token_file.parent().unwrap(),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(lock(&c.token_file).is_err());
    }
}
