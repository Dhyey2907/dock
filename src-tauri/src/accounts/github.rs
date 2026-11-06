//! GitHub Actions status: a personal access token (in Credential Manager) and a list of repos
//! (in Settings). Shows each repo's latest workflow run, checked every few minutes while the
//! widget is set up.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use super::{key, problem, KeyAccount};
use crate::{
    error::{AppError, AppResult},
    settings::{github_repo, SettingsStore},
};

pub const WIDGET_GITHUB: &str = "widget-github";
const API: &str = "https://api.github.com";
const REFRESH_EVERY: Duration = Duration::from_secs(3 * 60);

/// How a run is doing, reduced to what the widget shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RunState {
    Running,
    Passed,
    Failed,
    /// Cancelled, skipped or neutral.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoRun {
    pub repo: String,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<RunState>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    /// Why this repo couldn't be read.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubState {
    pub runs: Vec<RepoRun>,
}

#[derive(Default)]
pub struct GithubService {
    state: std::sync::Mutex<GithubState>,
    refresh_now: std::sync::Arc<tokio::sync::Notify>,
}

pub fn state_of(status: &str, conclusion: Option<&str>) -> RunState {
    if status != "completed" {
        return RunState::Running;
    }
    match conclusion {
        Some("success") => RunState::Passed,
        Some("failure" | "timed_out" | "startup_failure" | "action_required") => RunState::Failed,
        _ => RunState::Other,
    }
}

#[derive(Deserialize)]
struct Runs {
    #[serde(default)]
    workflow_runs: Vec<RawRun>,
}

#[derive(Deserialize)]
struct RawRun {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    head_branch: Option<String>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

/// The latest run in a `/actions/runs` answer.
pub fn parse_latest(repo: &str, json: &str) -> AppResult<RepoRun> {
    let runs: Runs = serde_json::from_str(json)?;
    let Some(run) = runs.workflow_runs.into_iter().next() else {
        return Ok(RepoRun {
            repo: repo.to_string(),
            workflow: None,
            branch: None,
            state: None,
            link: None,
            updated: None,
            problem: Some("no workflow runs yet".into()),
        });
    };
    Ok(RepoRun {
        repo: repo.to_string(),
        state: Some(state_of(&run.status, run.conclusion.as_deref())),
        workflow: run.name,
        branch: run.head_branch,
        link: run.html_url.filter(|u| u.starts_with("https://github.com/")),
        updated: run.updated_at,
        problem: None,
    })
}

async fn latest(http: &reqwest::Client, token: &str, repo: &str) -> RepoRun {
    let failed = |why: String| RepoRun {
        repo: repo.to_string(),
        workflow: None,
        branch: None,
        state: None,
        link: None,
        updated: None,
        problem: Some(why),
    };
    let Some(repo) = github_repo(repo) else {
        return failed("not an owner/name".into());
    };
    let response = http
        .get(format!("{API}/repos/{repo}/actions/runs"))
        .query(&[("per_page", "1")])
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await;
    let response = match response {
        Ok(r) => r,
        Err(_) => return failed("couldn't reach GitHub".into()),
    };
    match response.status().as_u16() {
        200 => {}
        404 => return failed("not found, or the token can't see it".into()),
        401 => return failed("the token wasn't accepted".into()),
        code => return failed(problem("GitHub", code).to_string().replace("invalid input: ", "")),
    }
    match response.text().await.map(|t| parse_latest(&repo, &t)) {
        Ok(Ok(run)) => run,
        _ => failed("GitHub's answer couldn't be read".into()),
    }
}

/// Every listed repo's latest run.
pub async fn runs(app: &AppHandle) -> AppResult<GithubState> {
    let token = key(KeyAccount::Github)
        .ok_or_else(|| AppError::InvalidInput("Add a GitHub token in Settings → Accounts.".into()))?;
    let repos = app.state::<SettingsStore>().get().accounts.github_repos;
    let http = super::http()?;
    let mut state = GithubState::default();
    for repo in repos {
        state.runs.push(latest(&http, &token, &repo).await);
    }
    let service = app.state::<GithubService>();
    *service.state.lock().expect("github lock") = state.clone();
    let _ = app.emit(WIDGET_GITHUB, &state);
    Ok(state)
}

pub fn current(app: &AppHandle) -> GithubState {
    app.state::<GithubService>()
        .state
        .lock()
        .expect("github lock")
        .clone()
}

/// Check again now (the token or repos changed).
pub fn refresh_soon(app: &AppHandle) {
    app.state::<GithubService>().refresh_now.notify_one();
}

pub fn init(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let wake = app.state::<GithubService>().refresh_now.clone();
        loop {
            let enabled = app
                .state::<SettingsStore>()
                .get()
                .widgets
                .iter()
                .any(|w| w.kind == crate::settings::WidgetKind::GithubActions && w.enabled);
            if enabled && key(KeyAccount::Github).is_some() {
                if let Err(error) = runs(&app).await {
                    tracing::info!(%error, "GitHub runs not read");
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(REFRESH_EVERY) => {}
                _ = wake.notified() => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_states_are_reduced() {
        assert_eq!(state_of("in_progress", None), RunState::Running);
        assert_eq!(state_of("queued", None), RunState::Running);
        assert_eq!(state_of("completed", Some("success")), RunState::Passed);
        assert_eq!(state_of("completed", Some("failure")), RunState::Failed);
        assert_eq!(state_of("completed", Some("cancelled")), RunState::Other);
    }

    #[test]
    fn the_latest_run_is_read() {
        let json = r#"{"total_count":2,"workflow_runs":[{"name":"CI","head_branch":"main","status":"completed",
            "conclusion":"failure","html_url":"https://github.com/a/b/actions/runs/1","updated_at":"2026-11-06T08:00:00Z"}]}"#;
        let run = parse_latest("a/b", json).unwrap();
        assert_eq!(run.state, Some(RunState::Failed));
        assert_eq!(run.branch.as_deref(), Some("main"));
        assert!(run.link.is_some());
        let none = parse_latest("a/b", r#"{"total_count":0,"workflow_runs":[]}"#).unwrap();
        assert!(none.problem.is_some());
    }
}
