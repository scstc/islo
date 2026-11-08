//! Microsoft public-client OAuth with PKCE. No codes or tokens enter Agent output.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};

const AUTHORIZE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "offline_access https://outlook.office.com/IMAP.AccessAsUser.All";
const CALLBACK: &str = "/oauth/callback";
pub(crate) const REGISTRATION_HELP: &str =
    "https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app";

pub(crate) fn valid_client_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn random() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "无法创建安全授权请求")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()).as_ref())
}
fn authorize_url(
    client_id: &str,
    email: &str,
    redirect: &str,
    state: &str,
    verifier: &str,
) -> Result<String, String> {
    if !valid_client_id(client_id) {
        return Err("请配置有效的 Microsoft 应用客户端 ID".into());
    }
    let mut url = url::Url::parse(AUTHORIZE).unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect),
        ("response_mode", "query"),
        ("scope", SCOPE),
        ("state", state),
        ("code_challenge", &challenge(verifier)),
        ("code_challenge_method", "S256"),
        ("login_hint", email),
        ("prompt", "select_account"),
    ]);
    Ok(url.into())
}

// Deliberately no Debug/Serialize: access tokens are ephemeral and never returned.
pub(crate) struct Authorized {
    pub access_token: String,
    pub refresh_token: String,
    pub client_id: String,
}
#[derive(Serialize, Deserialize)]
struct Stored {
    client_id: String,
    refresh_token: String,
}
impl Authorized {
    pub(crate) fn credential(&self) -> Result<String, String> {
        serde_json::to_string(&Stored {
            client_id: self.client_id.clone(),
            refresh_token: self.refresh_token.clone(),
        })
        .map_err(|_| "无法保存微软授权".into())
    }
}
fn token_response(
    raw: &str,
    client_id: &str,
    old_refresh: Option<&str>,
) -> Result<Authorized, String> {
    let json: serde_json::Value = serde_json::from_str(raw).map_err(|_| "微软授权响应无效")?;
    if let Some(error) = json["error"].as_str() {
        return Err(match error {
            "access_denied" => "已取消微软授权",
            "invalid_grant" | "interaction_required" | "login_required" => {
                "微软授权已失效，请重新登录 Outlook"
            }
            "invalid_client" | "unauthorized_client" => {
                "Microsoft 应用配置无效，请检查客户端 ID 与桌面重定向地址"
            }
            _ => "微软授权未完成，请重试",
        }
        .into());
    }
    let access = json["access_token"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 32768 && !s.chars().any(char::is_control))
        .ok_or("微软未返回有效的访问令牌")?;
    if !json["token_type"]
        .as_str()
        .is_some_and(|s| s.eq_ignore_ascii_case("Bearer"))
    {
        return Err("微软授权令牌类型无效".into());
    }
    if json["scope"].as_str().is_some_and(|s| {
        !s.split_whitespace().any(|scope| {
            scope.eq_ignore_ascii_case("https://outlook.office.com/IMAP.AccessAsUser.All")
                || scope.eq_ignore_ascii_case("IMAP.AccessAsUser.All")
        })
    }) {
        return Err("微软未授予 IMAP 邮件访问权限".into());
    }
    let refresh = json["refresh_token"]
        .as_str()
        .or(old_refresh)
        .filter(|s| !s.is_empty() && s.len() <= 32768 && !s.chars().any(char::is_control))
        .ok_or("微软未返回刷新令牌，请重新授权")?;
    Ok(Authorized {
        access_token: access.into(),
        refresh_token: refresh.into(),
        client_id: client_id.into(),
    })
}
fn token_request(params: &[(&str, &str)]) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(25))
        .build();
    let response = match agent.post(TOKEN).send_form(params) {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(_) => return Err("无法连接微软授权服务，请检查网络".into()),
    };
    let mut raw = String::new();
    response
        .into_reader()
        .take(131073)
        .read_to_string(&mut raw)
        .map_err(|_| "无法读取微软授权响应")?;
    if raw.len() > 131072 {
        return Err("微软授权响应过长".into());
    }
    Ok(raw)
}

pub(crate) fn has_credentials(
    account: &crate::mail::Account,
    client_id: &str,
) -> Result<bool, String> {
    Ok(crate::mail::secrets::get(&account.credential_key())?
        .and_then(|s| serde_json::from_str::<Stored>(&s).ok())
        .is_some_and(|s| {
            s.client_id.eq_ignore_ascii_case(client_id) && !s.refresh_token.is_empty()
        }))
}
pub(crate) fn access_token(account: &crate::mail::Account) -> Result<String, String> {
    let key = account.credential_key();
    let old = crate::mail::secrets::load(&key)?;
    let stored: Stored = serde_json::from_str(&old).map_err(|_| "请重新登录 Outlook")?;
    let configured = crate::model::Settings::load()
        .outlook_client_id()
        .to_string();
    if !valid_client_id(&stored.client_id) || !stored.client_id.eq_ignore_ascii_case(&configured) {
        return Err("Microsoft 应用已变更，请重新登录 Outlook".into());
    }
    let raw = token_request(&[
        ("client_id", &stored.client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", &stored.refresh_token),
        ("scope", SCOPE),
    ])?;
    let next = token_response(&raw, &stored.client_id, Some(&stored.refresh_token))?;
    // CAS prevents a late refresh from resurrecting a removed account or replacing new consent.
    crate::mail::secrets::replace_if_matches(&key, &old, &next.credential()?)?;
    Ok(next.access_token)
}

enum Callback {
    Code(String),
    Denied,
    Failed,
}
fn parse_callback(target: &str, state: &str) -> Result<Callback, String> {
    let url =
        url::Url::parse(&format!("http://localhost{target}")).map_err(|_| "Invalid callback")?;
    if url.path() != CALLBACK || url.host_str() != Some("localhost") {
        return Err("Invalid callback path".into());
    }
    let mut fields = std::collections::HashMap::new();
    for (key, value) in url.query_pairs() {
        if fields
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Err("Duplicate callback field".into());
        }
    }
    let actual = fields.get("state").ok_or("Missing callback state")?;
    #[allow(deprecated)]
    ring::constant_time::verify_slices_are_equal(actual.as_bytes(), state.as_bytes())
        .map_err(|_| "Invalid callback state")?;
    if let Some(error) = fields.get("error") {
        return Ok(if error == "access_denied" {
            Callback::Denied
        } else {
            Callback::Failed
        });
    }
    let code = fields
        .remove("code")
        .filter(|s| !s.is_empty() && s.len() <= 8192 && !s.chars().any(char::is_control))
        .ok_or("Invalid callback code")?;
    Ok(Callback::Code(code))
}
fn receive_callback(mut stream: TcpStream, port: u16, state: &str) -> Result<Callback, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| "Cannot read callback")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| "Cannot write callback")?;
    let mut header = Vec::new();
    let mut one = [0u8; 1];
    let reading = Instant::now();
    while header.len() < 16384 && !header.ends_with(b"\r\n\r\n") {
        if reading.elapsed() > Duration::from_secs(2) {
            return Err("Callback request timed out".into());
        }
        if stream.read(&mut one).map_err(|_| "Cannot read callback")? == 0 {
            break;
        }
        header.push(one[0]);
    }
    let text = std::str::from_utf8(&header).map_err(|_| "Invalid callback request")?;
    let mut lines = text.split("\r\n");
    let mut request = lines.next().unwrap_or("").split_whitespace();
    let method = request.next().unwrap_or("");
    let target = request.next().unwrap_or("");
    let hosts: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim())
        .collect();
    let result = if method == "GET"
        && target.starts_with("/oauth/callback?")
        && hosts == [format!("localhost:{port}").as_str()]
        && header.ends_with(b"\r\n\r\n")
    {
        parse_callback(target, state)
    } else {
        Err("Invalid callback request".into())
    };
    let (status, body) = if result.is_ok() {
        (
            "200 OK",
            "islo: Authorization received. You may close this page and return to islo.",
        )
    } else {
        ("400 Bad Request", "islo: Invalid authorization response.")
    };
    let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len());
    result
}

pub(crate) struct Job {
    cancel: Arc<AtomicBool>,
    receiver: Receiver<Result<Authorized, String>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}
impl Job {
    pub(crate) fn start(
        account: crate::mail::Account,
        client_id: String,
    ) -> Result<(Self, String), String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| "无法创建本机授权回调")?;
        listener
            .set_nonblocking(true)
            .map_err(|_| "无法创建本机授权回调")?;
        let port = listener
            .local_addr()
            .map_err(|_| "无法创建本机授权回调")?
            .port();
        let redirect = format!("http://localhost:{port}{CALLBACK}");
        let state = random()?;
        let verifier = random()?;
        let url = authorize_url(&client_id, &account.email, &redirect, &state, &verifier)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("islo-outlook-auth".into())
            .spawn(move || {
                let started = Instant::now();
                let result = (|| {
                    let code = loop {
                        if stop.load(Ordering::SeqCst) {
                            return Err("已取消微软授权".into());
                        }
                        if started.elapsed() > Duration::from_secs(900) {
                            return Err("微软授权已超时，请重试".into());
                        }
                        match listener.accept() {
                            Ok((stream, peer)) if peer.ip().is_loopback() => {
                                match receive_callback(stream, port, &state) {
                                    Ok(Callback::Code(code)) => break code,
                                    Ok(Callback::Denied) => return Err("已取消微软授权".into()),
                                    Ok(Callback::Failed) => {
                                        return Err("微软授权未完成，请重试".into())
                                    }
                                    Err(_) => {}
                                }
                            }
                            Ok(_) => {}
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(100))
                            }
                            Err(_) => return Err("无法接收微软授权回调".into()),
                        }
                    };
                    if stop.load(Ordering::SeqCst) {
                        return Err("已取消微软授权".into());
                    }
                    let raw = token_request(&[
                        ("client_id", &client_id),
                        ("grant_type", "authorization_code"),
                        ("code", &code),
                        ("redirect_uri", &redirect),
                        ("code_verifier", &verifier),
                        ("scope", SCOPE),
                    ])?;
                    let tokens = token_response(&raw, &client_id, None)?;
                    #[cfg(windows)]
                    crate::mail::net::probe(&account, &tokens.access_token).map_err(|_| {
                        "无法验证 Outlook 邮箱，请确认授权账号与邮箱一致，并已开启 IMAP"
                    })?;
                    #[cfg(not(windows))]
                    return Err("Outlook 邮箱接入需要 Windows".into());
                    if stop.load(Ordering::SeqCst) {
                        return Err("已取消微软授权".into());
                    }
                    Ok(tokens)
                })();
                if !stop.load(Ordering::SeqCst) {
                    let _ = sender.send(result);
                }
            })
            .map_err(|_| "无法启动微软授权")?;
        Ok((Self { cancel, receiver }, url))
    }
    pub(crate) fn poll(&self) -> Option<Result<Authorized, String>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("微软授权已中断".into())),
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CLIENT: &str = "12345678-1234-1234-1234-123456789abc";
    #[test]
    fn outlook_callback_http_never_reflects_codes_or_accepts_a_foreign_host() {
        for host in ["localhost", "attacker.example"] {
            let expected = host == "localhost";
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let host = host.to_string();
            let client = std::thread::spawn(move || {
                let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(4)))
                    .unwrap();
                write!(stream,"GET /oauth/callback?state=ok&code=SENSITIVE_TEST_CODE HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n").unwrap();
                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();
                response
            });
            let (stream, _) = listener.accept().unwrap();
            let result = receive_callback(stream, port, "ok");
            assert_eq!(result.is_ok(), expected);
            let response = client.join().unwrap();
            assert!(!response.contains("SENSITIVE_TEST_CODE"));
            assert!(response.contains("Cache-Control: no-store"));
        }
    }
    #[test]
    #[cfg(windows)]
    fn outlook_refresh_cannot_overwrite_new_consent_or_restore_a_deleted_credential() {
        let key = format!(
            "outlook-islo-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                crate::mail::secrets::delete(&self.0);
            }
        }
        let _cleanup = Cleanup(key.clone());
        crate::mail::secrets::save(&key, "old-synthetic").unwrap();
        crate::mail::secrets::save(&key, "new-synthetic").unwrap();
        assert!(
            crate::mail::secrets::replace_if_matches(&key, "old-synthetic", "late-synthetic")
                .is_err()
        );
        assert_eq!(crate::mail::secrets::load(&key).unwrap(), "new-synthetic");
        crate::mail::secrets::remove(&key).unwrap();
        assert!(
            crate::mail::secrets::replace_if_matches(&key, "new-synthetic", "late-synthetic")
                .is_err()
        );
        assert!(crate::mail::secrets::get(&key).unwrap().is_none());
    }
    #[test]
    fn outlook_pkce_uses_s256_and_only_mail_scopes() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let url = url::Url::parse(
            &authorize_url(
                CLIENT,
                "sample@outlook.com",
                "http://localhost:1234/oauth/callback",
                "state",
                "verifier",
            )
            .unwrap(),
        )
        .unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(query["scope"], SCOPE);
        assert_eq!(query["code_challenge_method"], "S256");
        assert!(!query.contains_key("client_secret"));
        assert!(!query.contains_key("code_verifier"));
        assert!(valid_client_id(CLIENT));
        assert!(!valid_client_id("not-an-app-id"));
    }
    #[test]
    fn outlook_callback_rejects_wrong_state_duplicates_and_wrong_path() {
        for path in [
            "/oauth/callback?state=wrong&code=secret",
            "/oauth/callback?state=ok&state=ok&code=secret",
            "/other?state=ok&code=secret",
        ] {
            assert!(parse_callback(path, "ok").is_err());
        }
        assert!(matches!(
            parse_callback("/oauth/callback?state=ok&code=secret", "ok"),
            Ok(Callback::Code(_))
        ));
        assert!(matches!(
            parse_callback(
                "/oauth/callback?state=ok&error=access_denied&error_description=SECRET",
                "ok"
            ),
            Ok(Callback::Denied)
        ));
    }
    #[test]
    fn outlook_tokens_rotate_refresh_without_leaking_remote_errors() {
        let result = token_response(
            r#"{"error":"invalid_grant","error_description":"SENSITIVE"}"#,
            CLIENT,
            None,
        )
        .err()
        .unwrap();
        assert!(!result.contains("SENSITIVE"));
        let token = token_response(
            r#"{"access_token":"access","token_type":"Bearer","refresh_token":"new"}"#,
            CLIENT,
            Some("old"),
        )
        .unwrap();
        assert_eq!(token.refresh_token, "new");
        assert!(!token.credential().unwrap().contains("access_token"));
        assert_eq!(
            token_response(
                r#"{"access_token":"access","token_type":"Bearer"}"#,
                CLIENT,
                Some("old")
            )
            .unwrap()
            .refresh_token,
            "old"
        );
        assert!(token_response(
            r#"{"access_token":"access","token_type":"Bearer"}"#,
            CLIENT,
            None
        )
        .is_err());
    }
}
