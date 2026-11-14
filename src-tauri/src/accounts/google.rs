//! Google Tasks: the user's own Google OAuth client ("Desktop app" type: its client id in
//! Settings, its client secret in Credential Manager), signed in with PKCE through a loopback
//! address on any free port. Shows open tasks from the first few lists and ticks them off.

use std::net::TcpListener;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::{key, oauth, problem, KeyAccount, OAuthProvider};
use crate::{
    error::{AppError, AppResult},
    settings::SettingsStore,
};

const API: &str = "https://tasks.googleapis.com/tasks/v1";
/// Tasks, and Gmail's labels (for the unread count on the Gmail card; no mail is read).
const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/tasks",
    "https://www.googleapis.com/auth/gmail.labels",
];
const MAX_LISTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GoogleTask {
    pub id: String,
    pub list_id: String,
    pub list: String,
    pub title: String,
    /// Due date (Google keeps only the day), "YYYY-MM-DD".
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

pub fn client(client_id: &str, secret: Option<String>) -> oauth::Client {
    oauth::Client {
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
        token_url: "https://oauth2.googleapis.com/token",
        client_id: client_id.trim().to_string(),
        client_secret: secret,
        scopes: SCOPES,
        // A refresh token, so Dock stays signed in.
        extra: &[("access_type", "offline"), ("prompt", "consent")],
    }
}

fn client_for(app: &AppHandle) -> AppResult<oauth::Client> {
    let id = app.state::<SettingsStore>().get().accounts.google_client_id;
    if id.is_empty() {
        return Err(AppError::InvalidInput(
            "Add your Google client ID in Settings → Accounts.".into(),
        ));
    }
    Ok(client(&id, key(KeyAccount::GoogleSecret)))
}

/// A fresh access token for the signed-in Google account.
pub async fn token(app: &AppHandle) -> AppResult<String> {
    let client = client_for(app)?;
    oauth::access_token(OAuthProvider::Google, &client).await
}

pub async fn sign_in(app: &AppHandle) -> AppResult<()> {
    let client = client_for(app)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let redirect = format!("http://127.0.0.1:{}/callback", listener.local_addr()?.port());
    oauth::sign_in(OAuthProvider::Google, client, listener, redirect).await
}

#[derive(Deserialize)]
struct Items<T> {
    #[serde(default = "Vec::new")]
    items: Vec<T>,
}

#[derive(Deserialize)]
struct RawList {
    id: String,
    #[serde(default)]
    title: String,
}

#[derive(Deserialize)]
struct RawTask {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    due: Option<String>,
}

/// Open tasks from one list's answer.
pub fn parse_tasks(json: &str, list_id: &str, list: &str) -> AppResult<Vec<GoogleTask>> {
    let page: Items<RawTask> = serde_json::from_str(json)?;
    Ok(page
        .items
        .into_iter()
        .filter(|t| t.status != "completed" && !t.title.trim().is_empty())
        .map(|t| GoogleTask {
            id: t.id,
            list_id: list_id.to_string(),
            list: list.to_string(),
            title: t.title,
            due: t.due.and_then(|d| d.get(..10).map(str::to_string)),
        })
        .collect())
}

/// `{API}/lists/{list}/tasks[/{task}]` with each id escaped.
pub fn task_url(list: &str, task: Option<&str>) -> String {
    let mut url = url::Url::parse(API).expect("tasks address");
    {
        let mut path = url.path_segments_mut().expect("tasks address has a path");
        path.push("lists").push(list).push("tasks");
        if let Some(task) = task {
            path.push(task);
        }
    }
    url.into()
}

pub async fn tasks(app: &AppHandle) -> AppResult<Vec<GoogleTask>> {
    let token = oauth::access_token(OAuthProvider::Google, &client_for(app)?).await?;
    let http = super::http()?;
    let lists = http
        .get(format!("{API}/users/@me/lists"))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(super::offline("Google Tasks"))?;
    if !lists.status().is_success() {
        return Err(problem("Google Tasks", lists.status().as_u16()));
    }
    let lists: Items<RawList> =
        serde_json::from_str(&lists.text().await.map_err(super::offline("Google Tasks"))?)?;
    let mut all = Vec::new();
    for list in lists.items.into_iter().take(MAX_LISTS) {
        let response = http
            .get(task_url(&list.id, None))
            .query(&[
                ("showCompleted", "false"),
                ("showHidden", "false"),
                ("maxResults", "50"),
            ])
            .bearer_auth(&token)
            .send()
            .await
            .map_err(super::offline("Google Tasks"))?;
        if !response.status().is_success() {
            return Err(problem("Google Tasks", response.status().as_u16()));
        }
        let text = response.text().await.map_err(super::offline("Google Tasks"))?;
        all.extend(parse_tasks(&text, &list.id, &list.title)?);
    }
    Ok(all)
}

pub async fn complete(app: &AppHandle, list_id: &str, task_id: &str) -> AppResult<()> {
    let token = oauth::access_token(OAuthProvider::Google, &client_for(app)?).await?;
    let response = super::http()?
        .patch(task_url(list_id, Some(task_id)))
        .bearer_auth(token)
        .json(&json!({ "status": "completed" }))
        .send()
        .await
        .map_err(super::offline("Google Tasks"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(problem("Google Tasks", response.status().as_u16()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sign_in_address_asks_to_stay_signed_in() {
        let pkce = oauth::Pkce {
            verifier: "v".into(),
            challenge: "c".into(),
            state: "s".into(),
        };
        let address = oauth::authorize_url(
            &client("123.apps.googleusercontent.com", None),
            "http://127.0.0.1:51234/callback",
            &pkce,
        );
        assert!(address.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert!(address.contains("access_type=offline"));
        assert!(address.contains("scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Ftasks"));
        assert!(address.contains("code_challenge_method=S256"));
    }

    #[test]
    fn open_tasks_are_read_and_ids_escaped() {
        let json = r#"{"items":[
            {"id":"t1","title":"Pay rent","status":"needsAction","due":"2026-11-30T00:00:00.000Z"},
            {"id":"t2","title":"Done one","status":"completed"},
            {"id":"t3","title":"  ","status":"needsAction"}]}"#;
        let tasks = parse_tasks(json, "L1", "My Tasks").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].due.as_deref(), Some("2026-11-30"));
        assert!(parse_tasks("{}", "L", "x").unwrap().is_empty());
        assert_eq!(
            task_url("a/b", Some("c?d")),
            "https://tasks.googleapis.com/tasks/v1/lists/a%2Fb/tasks/c%3Fd"
        );
    }
}
