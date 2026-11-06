//! Spotify playlists: the user's own Spotify app (its client id in Settings), signed in with
//! PKCE through a fixed loopback address they add to that app. Lists their playlists and plays
//! one on the active device (Spotify only allows that with Premium).

use std::net::TcpListener;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::{oauth, problem, OAuthProvider};
use crate::{
    error::{AppError, AppResult},
    settings::SettingsStore,
};

/// Spotify wants the exact redirect address registered, port and all.
pub const PORT: u16 = 43821;
pub const REDIRECT_URI: &str = "http://127.0.0.1:43821/callback";
const API: &str = "https://api.spotify.com/v1";
const SCOPES: &[&str] = &[
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-read-playback-state",
    "user-modify-playback-state",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    /// "spotify:playlist:…", passed back to play it.
    pub uri: String,
    /// Who made it.
    pub owner: String,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<u32>,
    /// Its page on open.spotify.com.
    pub link: String,
}

pub fn client(client_id: &str) -> oauth::Client {
    oauth::Client {
        auth_url: "https://accounts.spotify.com/authorize",
        token_url: "https://accounts.spotify.com/api/token",
        client_id: client_id.trim().to_string(),
        client_secret: None,
        scopes: SCOPES,
        extra: &[],
    }
}

fn client_for(app: &AppHandle) -> AppResult<oauth::Client> {
    let id = app.state::<SettingsStore>().get().accounts.spotify_client_id;
    if id.is_empty() {
        return Err(AppError::InvalidInput(
            "Add your Spotify app's client ID in Settings → Accounts.".into(),
        ));
    }
    Ok(client(&id))
}

pub async fn sign_in(app: &AppHandle) -> AppResult<()> {
    let client = client_for(app)?;
    let listener = TcpListener::bind(("127.0.0.1", PORT)).map_err(|_| {
        AppError::InvalidInput(format!(
            "Port {PORT} is busy. Close what's using it and try again."
        ))
    })?;
    oauth::sign_in(OAuthProvider::Spotify, client, listener, REDIRECT_URI.into()).await
}

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    items: Vec<RawPlaylist>,
}

#[derive(Deserialize)]
struct RawPlaylist {
    id: String,
    #[serde(default)]
    name: String,
    uri: String,
    #[serde(default)]
    owner: Option<serde_json::Value>,
    #[serde(default)]
    tracks: Option<serde_json::Value>,
    #[serde(default)]
    external_urls: Option<serde_json::Value>,
}

/// Playlists from one page of `/me/playlists`.
pub fn parse_playlists(json: &str) -> AppResult<Vec<Playlist>> {
    let page: Page = serde_json::from_str(json)?;
    Ok(page
        .items
        .into_iter()
        .filter(|p| p.uri.starts_with("spotify:playlist:"))
        .map(|p| Playlist {
            owner: p
                .owner
                .as_ref()
                .and_then(|o| o["display_name"].as_str())
                .unwrap_or_default()
                .to_string(),
            tracks: p
                .tracks
                .as_ref()
                .and_then(|t| t["total"].as_u64())
                .map(|n| n as u32),
            link: p
                .external_urls
                .as_ref()
                .and_then(|u| u["spotify"].as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("https://open.spotify.com/playlist/{}", p.id)),
            id: p.id,
            name: if p.name.is_empty() {
                "Untitled".into()
            } else {
                p.name
            },
            uri: p.uri,
        })
        .collect())
}

/// Whether `uri` is a playlist address Dock may ask Spotify to play.
pub fn valid_uri(uri: &str) -> bool {
    uri.strip_prefix("spotify:playlist:")
        .is_some_and(|id| !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric()))
}

pub async fn playlists(app: &AppHandle) -> AppResult<Vec<Playlist>> {
    let token = oauth::access_token(OAuthProvider::Spotify, &client_for(app)?).await?;
    let response = super::http()?
        .get(format!("{API}/me/playlists?limit=50"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(super::offline("Spotify"))?;
    if !response.status().is_success() {
        return Err(problem("Spotify", response.status().as_u16()));
    }
    parse_playlists(&response.text().await.map_err(super::offline("Spotify"))?)
}

pub async fn play(app: &AppHandle, uri: &str) -> AppResult<()> {
    if !valid_uri(uri) {
        return Err(AppError::InvalidInput("not a playlist".into()));
    }
    let token = oauth::access_token(OAuthProvider::Spotify, &client_for(app)?).await?;
    let http = super::http()?;
    let body = json!({ "context_uri": uri });
    let first = http
        .put(format!("{API}/me/player/play"))
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await
        .map_err(super::offline("Spotify"))?;
    match first.status().as_u16() {
        200..=299 => return Ok(()),
        403 => {
            return Err(AppError::InvalidInput(
                "Spotify only lets apps start music with Premium.".into(),
            ))
        }
        404 => {}
        code => return Err(problem("Spotify", code)),
    }
    // No active device: wake the first one Spotify knows about.
    let devices: serde_json::Value = http
        .get(format!("{API}/me/player/devices"))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(super::offline("Spotify"))?
        .json()
        .await
        .map_err(super::offline("Spotify"))?;
    let Some(device) = devices["devices"][0]["id"].as_str() else {
        return Err(AppError::InvalidInput(
            "Open Spotify on this PC or your phone first.".into(),
        ));
    };
    let mut address = url::Url::parse(&format!("{API}/me/player/play")).expect("spotify address");
    address.query_pairs_mut().append_pair("device_id", device);
    let second = http
        .put(address)
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await
        .map_err(super::offline("Spotify"))?;
    if second.status().is_success() {
        Ok(())
    } else {
        Err(problem("Spotify", second.status().as_u16()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sign_in_address_asks_for_playback_and_playlists() {
        let pkce = oauth::Pkce {
            verifier: "v".into(),
            challenge: "c".into(),
            state: "s".into(),
        };
        let address = oauth::authorize_url(&client(" my-id "), REDIRECT_URI, &pkce);
        assert!(address.starts_with("https://accounts.spotify.com/authorize?client_id=my-id&"));
        assert!(address.contains("user-modify-playback-state"));
        assert!(address.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A43821%2Fcallback"));
    }

    #[test]
    fn playlists_are_read() {
        let json = r#"{"items":[
            {"id":"37i9","name":"Focus","uri":"spotify:playlist:37i9","owner":{"display_name":"Spotify"},
             "tracks":{"total":80},"external_urls":{"spotify":"https://open.spotify.com/playlist/37i9"}},
            {"id":"x","name":"","uri":"spotify:album:x"}]}"#;
        let list = parse_playlists(json).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].owner, "Spotify");
        assert_eq!(list[0].tracks, Some(80));
        assert!(valid_uri(&list[0].uri));
        assert!(!valid_uri("spotify:playlist:../x"));
        assert!(!valid_uri("spotify:track:1"));
    }
}
