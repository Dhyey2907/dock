//! Signing in with OAuth 2 the way desktop apps should: authorization code with PKCE, the
//! browser sent back to a loopback address Dock listens on for one request. Tokens (with the
//! refresh token) are kept in Windows Credential Manager and refreshed when they expire.

use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{secret_delete, secret_get, secret_set, OAuthProvider};
use crate::error::{AppError, AppResult};

/// How long Dock waits for the browser to come back.
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Tokens are refreshed this long before they expire.
const EXPIRY_MARGIN_SECS: i64 = 60;

/// Everything one provider's sign-in needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub client_id: String,
    /// Google's "Desktop app" clients have a secret that isn't really secret but is required.
    pub client_secret: Option<String>,
    pub scopes: &'static [&'static str],
    /// Extra query parameters for the browser address.
    pub extra: &'static [(&'static str, &'static str)],
}

/// A PKCE pair and the anti-forgery `state`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// The S256 challenge for a verifier (RFC 7636).
pub fn challenge(verifier: &str) -> String {
    b64url(&Sha256::digest(verifier.as_bytes()))
}

fn random(len: usize) -> AppResult<Vec<u8>> {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes).map_err(|e| AppError::Platform(e.to_string()))?;
    Ok(bytes)
}

pub fn new_pkce() -> AppResult<Pkce> {
    let verifier = b64url(&random(48)?);
    Ok(Pkce {
        challenge: challenge(&verifier),
        verifier,
        state: b64url(&random(16)?),
    })
}

/// The address the browser opens to sign in.
pub fn authorize_url(client: &Client, redirect_uri: &str, pkce: &Pkce) -> String {
    let mut url = url::Url::parse(client.auth_url).expect("provider auth address");
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("client_id", &client.client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &pkce.challenge)
            .append_pair("state", &pkce.state)
            .append_pair("scope", &client.scopes.join(" "));
        for (key, value) in client.extra {
            query.append_pair(key, value);
        }
    }
    url.into()
}

/// What the browser brought back: the code, or why there isn't one.
pub fn parse_callback(target: &str, expected_state: &str) -> Result<String, String> {
    let url =
        url::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| "a broken answer".to_string())?;
    let get = |key: &str| {
        url.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };
    if let Some(error) = get("error") {
        return Err(if error == "access_denied" {
            "sign-in was cancelled".into()
        } else {
            format!("the sign-in failed ({error})")
        });
    }
    if get("state").as_deref() != Some(expected_state) {
        return Err("the answer didn't match this sign-in".into());
    }
    get("code")
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "no code came back".into())
}

/// A token as kept in Credential Manager.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

/// Read a token endpoint's answer; a refresh that brings no new refresh token keeps the old one.
pub fn parse_token(
    json: &str,
    now: DateTime<Utc>,
    previous_refresh: Option<String>,
) -> AppResult<StoredToken> {
    let response: TokenResponse = serde_json::from_str(json)?;
    Ok(StoredToken {
        access_token: response.access_token,
        refresh_token: response.refresh_token.or(previous_refresh),
        expires_at: now + chrono::Duration::seconds(response.expires_in.unwrap_or(3600)),
    })
}

fn secret_name(provider: OAuthProvider) -> &'static str {
    match provider {
        OAuthProvider::Spotify => "spotify-token",
        OAuthProvider::Google => "google-token",
    }
}

pub fn stored(provider: OAuthProvider) -> Option<StoredToken> {
    serde_json::from_str(&secret_get(secret_name(provider))?).ok()
}

fn store(provider: OAuthProvider, token: &StoredToken) -> AppResult<()> {
    secret_set(secret_name(provider), &serde_json::to_string(token)?)
}

pub fn forget(provider: OAuthProvider) {
    secret_delete(secret_name(provider));
}

const PAGE: &str = "<!doctype html><meta charset=utf-8><title>Dock</title>\
<body style=\"font:16px system-ui;padding:3em;text-align:center\"><h1>MESSAGE</h1>\
<p>You can close this tab and go back to Dock.</p></body>";

fn reply(stream: &mut std::net::TcpStream, status: &str, message: &str) {
    let body = PAGE.replace("MESSAGE", message);
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

/// Wait on `listener` for the browser to come back to `/callback`; other requests (the
/// browser's icon, say) get a 404.
fn wait_for_code(listener: TcpListener, state: &str) -> AppResult<String> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + SIGN_IN_TIMEOUT;
    while Instant::now() < deadline {
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut buf = [0u8; 8192];
        let read = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..read]);
        let target = request
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("GET "))
            .and_then(|rest| rest.split(' ').next())
            .unwrap_or("");
        if !target.starts_with("/callback") {
            reply(&mut stream, "404 Not Found", "Nothing here");
            continue;
        }
        return match parse_callback(target, state) {
            Ok(code) => {
                reply(&mut stream, "200 OK", "Signed in");
                Ok(code)
            }
            Err(why) => {
                reply(&mut stream, "400 Bad Request", "Sign-in didn't finish");
                Err(AppError::InvalidInput(format!("Sign-in didn't finish: {why}.")))
            }
        };
    }
    Err(AppError::InvalidInput("Sign-in timed out. Try again.".into()))
}

async fn exchange(
    client: &Client,
    form: &[(&str, &str)],
    previous_refresh: Option<String>,
) -> AppResult<StoredToken> {
    let mut form: Vec<(&str, &str)> = form.to_vec();
    form.push(("client_id", &client.client_id));
    if let Some(secret) = &client.client_secret {
        form.push(("client_secret", secret));
    }
    let response = super::http()?
        .post(client.token_url)
        .form(&form)
        .send()
        .await
        .map_err(super::offline("the sign-in service"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(super::offline("the sign-in service"))?;
    if !status.is_success() {
        tracing::info!(status = status.as_u16(), "token request refused");
        return Err(AppError::InvalidInput(
            if text.contains("invalid_client") || text.contains("client_secret") {
                "The client ID (or Google client secret) wasn't accepted. Check it in Settings → Accounts."
            } else {
                "The sign-in was refused. Try signing in again."
            }
            .into(),
        ));
    }
    parse_token(&text, Utc::now(), previous_refresh)
}

/// Sign in: open the browser, wait for it to come back to `listener`, trade the code for tokens
/// and keep them.
pub async fn sign_in(
    provider: OAuthProvider,
    client: Client,
    listener: TcpListener,
    redirect_uri: String,
) -> AppResult<()> {
    if client.client_id.trim().is_empty() {
        return Err(AppError::InvalidInput("Add your client ID first.".into()));
    }
    let pkce = new_pkce()?;
    let address = authorize_url(&client, &redirect_uri, &pkce);
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Program(address));
    #[cfg(not(windows))]
    let _ = address;
    let state = pkce.state.clone();
    let code = tauri::async_runtime::spawn_blocking(move || wait_for_code(listener, &state))
        .await
        .map_err(|e| AppError::Platform(e.to_string()))??;
    let token = exchange(
        &client,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
            ("code_verifier", &pkce.verifier),
        ],
        None,
    )
    .await?;
    store(provider, &token)?;
    tracing::info!(?provider, "signed in");
    Ok(())
}

/// A current access token, refreshed first if it has expired.
pub async fn access_token(provider: OAuthProvider, client: &Client) -> AppResult<String> {
    let not_signed_in = || AppError::InvalidInput("Not signed in. Sign in from Settings → Accounts.".into());
    let token = stored(provider).ok_or_else(not_signed_in)?;
    if token.expires_at > Utc::now() + chrono::Duration::seconds(EXPIRY_MARGIN_SECS) {
        return Ok(token.access_token);
    }
    let refresh = token.refresh_token.clone().ok_or_else(not_signed_in)?;
    let fresh = exchange(
        client,
        &[("grant_type", "refresh_token"), ("refresh_token", &refresh)],
        Some(refresh.clone()),
    )
    .await
    .inspect_err(|_| forget(provider))?;
    store(provider, &fresh)?;
    Ok(fresh.access_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> Client {
        Client {
            auth_url: "https://accounts.example.com/authorize",
            token_url: "https://accounts.example.com/token",
            client_id: "abc 123".into(),
            client_secret: None,
            scopes: &["read", "write:all"],
            extra: &[("access_type", "offline")],
        }
    }

    #[test]
    fn the_challenge_is_the_unpadded_url_safe_sha256() {
        assert_eq!(challenge(""), "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU");
        assert_eq!(challenge("abc"), "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0");
    }

    #[test]
    fn new_pairs_are_random_and_valid() {
        let a = new_pkce().unwrap();
        let b = new_pkce().unwrap();
        assert_ne!(a.verifier, b.verifier);
        assert!((43..=128).contains(&a.verifier.len()));
        assert_eq!(a.challenge, challenge(&a.verifier));
    }

    #[test]
    fn the_browser_address_carries_everything() {
        let pkce = Pkce {
            verifier: "v".into(),
            challenge: "CH".into(),
            state: "ST".into(),
        };
        let address = authorize_url(&client(), "http://127.0.0.1:4000/callback", &pkce);
        let url = url::Url::parse(&address).unwrap();
        let pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into(), v.into())).collect();
        let get = |key: &str| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        assert_eq!(url.host_str(), Some("accounts.example.com"));
        assert_eq!(get("client_id"), Some("abc 123"));
        assert_eq!(get("response_type"), Some("code"));
        assert_eq!(get("redirect_uri"), Some("http://127.0.0.1:4000/callback"));
        assert_eq!(get("code_challenge_method"), Some("S256"));
        assert_eq!(get("code_challenge"), Some("CH"));
        assert_eq!(get("state"), Some("ST"));
        assert_eq!(get("scope"), Some("read write:all"));
        assert_eq!(get("access_type"), Some("offline"));
        assert!(address.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A4000%2Fcallback"));
    }

    #[test]
    fn callbacks_are_checked() {
        assert_eq!(
            parse_callback("/callback?code=xyz&state=S", "S"),
            Ok("xyz".into())
        );
        assert!(parse_callback("/callback?code=xyz&state=other", "S").is_err());
        assert!(parse_callback("/callback?error=access_denied&state=S", "S")
            .unwrap_err()
            .contains("cancelled"));
        assert!(parse_callback("/callback?state=S", "S").is_err());
    }

    #[test]
    fn token_answers_keep_the_refresh_token() {
        let now = Utc::now();
        let first = parse_token(
            r#"{"access_token":"a1","refresh_token":"r1","expires_in":3600,"token_type":"Bearer"}"#,
            now,
            None,
        )
        .unwrap();
        assert_eq!(first.refresh_token.as_deref(), Some("r1"));
        assert_eq!(first.expires_at, now + chrono::Duration::seconds(3600));
        let refreshed = parse_token(
            r#"{"access_token":"a2","expires_in":60}"#,
            now,
            first.refresh_token,
        )
        .unwrap();
        assert_eq!(refreshed.access_token, "a2");
        assert_eq!(refreshed.refresh_token.as_deref(), Some("r1"));
        let json = serde_json::to_string(&refreshed).unwrap();
        assert_eq!(serde_json::from_str::<StoredToken>(&json).unwrap(), refreshed);
    }
}
