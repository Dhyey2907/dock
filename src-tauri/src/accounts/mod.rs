//! Widgets that use the user's own accounts: Ask Claude (an Anthropic API key), Spotify and
//! Google Tasks (OAuth with the user's own app, PKCE and a loopback redirect), Notion (an
//! integration token) and GitHub Actions (a personal token).
//!
//! Every key and token is kept in Windows Credential Manager, never in the settings file, the
//! log or the page: the page only learns whether each account is set up.

pub mod anthropic;
pub mod github;
pub mod google;
pub mod notion;
pub mod oauth;
pub mod spotify;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

const KEYRING_SERVICE: &str = "com.dhyey.dock";

/// A secret the user pastes in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KeyAccount {
    Anthropic,
    Notion,
    Github,
    /// The client secret Google gives "Desktop app" clients (Google asks for it even with PKCE).
    GoogleSecret,
}

/// An account signed in through the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OAuthProvider {
    Spotify,
    Google,
}

/// Which accounts are set up (never the secrets themselves).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccountStatus {
    pub anthropic: bool,
    pub notion: bool,
    pub github: bool,
    pub google_secret: bool,
    /// Signed in to Spotify.
    pub spotify: bool,
    /// Signed in to Google.
    pub google: bool,
    /// The loopback address to add to the Spotify app's redirect URIs.
    pub spotify_redirect: String,
}

fn entry(name: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("account/{name}"))
        .map_err(|e| AppError::Platform(e.to_string()))
}

pub fn secret_get(name: &str) -> Option<String> {
    entry(name).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

pub fn secret_set(name: &str, value: &str) -> AppResult<()> {
    entry(name)?
        .set_password(value)
        .map_err(|e| AppError::Platform(e.to_string()))
}

pub fn secret_delete(name: &str) {
    if let Ok(entry) = entry(name) {
        // Already gone is fine.
        let _ = entry.delete_credential();
    }
}

impl KeyAccount {
    fn secret_name(self) -> &'static str {
        match self {
            KeyAccount::Anthropic => "anthropic",
            KeyAccount::Notion => "notion",
            KeyAccount::Github => "github",
            KeyAccount::GoogleSecret => "google-secret",
        }
    }
}

/// Check a pasted key looks like the kind asked for, so a slip (pasting the wrong thing) is
/// caught before it's saved. Returns it trimmed.
pub fn check_key(account: KeyAccount, value: &str) -> AppResult<String> {
    let value = value.trim();
    let bad = |what: &str| Err(AppError::InvalidInput(what.to_string()));
    if value.len() > 400 || value.chars().any(char::is_whitespace) {
        return bad("That doesn't look like a key. Paste just the key.");
    }
    match account {
        KeyAccount::Anthropic if !value.starts_with("sk-ant-") => {
            bad("Anthropic API keys start with \"sk-ant-\". Create one at console.anthropic.com.")
        }
        KeyAccount::Notion if !(value.starts_with("ntn_") || value.starts_with("secret_")) => {
            bad("Notion integration tokens start with \"ntn_\" (or \"secret_\" for older ones).")
        }
        KeyAccount::Github
            if !(value.starts_with("ghp_")
                || value.starts_with("github_pat_")
                || value.starts_with("gho_")) =>
        {
            bad("GitHub tokens start with \"github_pat_\" or \"ghp_\".")
        }
        _ if value.len() < 8 => bad("That key is too short."),
        _ => Ok(value.to_string()),
    }
}

/// Save a pasted key; an empty one removes it.
pub fn set_key(account: KeyAccount, value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        secret_delete(account.secret_name());
        return Ok(());
    }
    let value = check_key(account, value)?;
    secret_set(account.secret_name(), &value)
}

pub fn key(account: KeyAccount) -> Option<String> {
    secret_get(account.secret_name())
}

pub fn status() -> AccountStatus {
    AccountStatus {
        anthropic: key(KeyAccount::Anthropic).is_some(),
        notion: key(KeyAccount::Notion).is_some(),
        github: key(KeyAccount::Github).is_some(),
        google_secret: key(KeyAccount::GoogleSecret).is_some(),
        spotify: oauth::stored(OAuthProvider::Spotify).is_some(),
        google: oauth::stored(OAuthProvider::Google).is_some(),
        spotify_redirect: spotify::REDIRECT_URI.to_string(),
    }
}

pub fn disconnect(provider: OAuthProvider) {
    oauth::forget(provider);
}

/// The HTTP client the account widgets share.
pub fn http() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Dock (Windows desktop)")
        .build()
        .map_err(|e| AppError::Platform(e.to_string()))
}

/// A sentence for people from a failed request: the service, and what went wrong.
pub fn problem(service: &str, status: u16) -> AppError {
    AppError::InvalidInput(match status {
        401 => format!("{service} didn't accept the saved key or sign-in. Check it in Settings → Accounts."),
        403 => format!("{service} said this account isn't allowed to do that."),
        404 => format!("{service} couldn't find that. Check the ID in Settings → Accounts."),
        429 => format!("{service} is limiting requests. Try again in a minute."),
        500..=599 => format!("{service} is having trouble right now. Try again soon."),
        code => format!("{service} answered with error {code}."),
    })
}

pub fn offline(service: &str) -> impl Fn(reqwest::Error) -> AppError + '_ {
    move |_| AppError::InvalidInput(format!("Couldn't reach {service}. Check your connection."))
}

/// Links the widgets may open in the browser.
pub fn open_link(url: &str) -> AppResult<()> {
    let parsed = url::Url::parse(url).map_err(|_| AppError::InvalidInput("not a link".into()))?;
    let host = parsed.host_str().unwrap_or_default();
    let allowed = [
        "github.com",
        "www.notion.so",
        "notion.so",
        "open.spotify.com",
        "tasks.google.com",
    ];
    if parsed.scheme() != "https" || !allowed.contains(&host) {
        return Err(AppError::InvalidInput("not a link Dock opens".into()));
    }
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Program(parsed.to_string()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_keys_are_checked_by_kind() {
        assert!(check_key(KeyAccount::Anthropic, " sk-ant-api03-abcdefgh ").is_ok());
        assert!(check_key(KeyAccount::Anthropic, "sk-proj-123456789").is_err());
        assert!(check_key(KeyAccount::Notion, "ntn_123456789").is_ok());
        assert!(check_key(KeyAccount::Github, "github_pat_11ABC").is_ok());
        assert!(check_key(KeyAccount::Github, "two words").is_err());
        assert!(check_key(KeyAccount::GoogleSecret, "GOCSPX-abcdefgh").is_ok());
    }

    #[test]
    fn only_known_sites_open() {
        assert!(open_link("http://github.com/a/b").is_err());
        assert!(open_link("https://evil.example/").is_err());
        assert!(open_link("not a link").is_err());
    }
}
