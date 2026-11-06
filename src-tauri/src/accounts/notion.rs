//! Notion tasks: an internal integration's token (in Credential Manager) and the database the
//! tasks live in (its id in Settings). Shows the tasks not yet done and ticks them off. A task
//! is done by a checkbox property ("Done", "Complete" or the first checkbox), or else by a
//! status property in its "Complete" group.

use serde::Serialize;
use serde_json::{json, Value};
use ts_rs::TS;

use super::{key, problem, KeyAccount};
use crate::error::{AppError, AppResult};

const API: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotionTask {
    pub id: String,
    pub title: String,
    pub link: String,
}

/// How a database marks a task done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoneBy {
    Checkbox(String),
    /// The status property, and the option that means done.
    Status {
        property: String,
        done: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub title: String,
    pub done: DoneBy,
}

/// The 32-hex database id from a pasted id or link.
pub fn database_id(text: &str) -> Option<String> {
    let text = text.trim();
    let path = text.split(['?', '#']).next().unwrap_or(text);
    let candidate = path.rsplit(['/', '-']).next().unwrap_or(path).replace('-', "");
    let hex: String = candidate.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() == 32 && candidate.len() == 32 {
        return Some(hex.to_lowercase());
    }
    // A dashed UUID pasted on its own.
    let plain = text.replace('-', "");
    (plain.len() == 32 && plain.chars().all(|c| c.is_ascii_hexdigit())).then(|| plain.to_lowercase())
}

/// Work out the title and done properties from a database's description.
pub fn schema(database: &Value) -> Option<Schema> {
    let properties = database["properties"].as_object()?;
    let title = properties
        .iter()
        .find(|(_, p)| p["type"] == "title")
        .map(|(name, _)| name.clone())?;
    let named =
        |name: &str| ["done", "complete", "completed", "finished"].contains(&name.to_lowercase().as_str());
    let checkboxes: Vec<&String> = properties
        .iter()
        .filter(|(_, p)| p["type"] == "checkbox")
        .map(|(name, _)| name)
        .collect();
    if let Some(name) = checkboxes.iter().find(|n| named(n)).or(checkboxes.first()) {
        return Some(Schema {
            title,
            done: DoneBy::Checkbox((*name).clone()),
        });
    }
    let (property, status) = properties.iter().find(|(_, p)| p["type"] == "status")?;
    let options = status["status"]["options"].as_array()?;
    let complete_ids: Vec<&str> = status["status"]["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| {
            g["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case("complete"))
        })
        .flat_map(|g| g["option_ids"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let done = options
        .iter()
        .find(|o| o["id"].as_str().is_some_and(|id| complete_ids.contains(&id)))
        .or_else(|| options.iter().find(|o| o["name"].as_str().is_some_and(named)))?["name"]
        .as_str()?
        .to_string();
    Some(Schema {
        title,
        done: DoneBy::Status {
            property: property.clone(),
            done,
        },
    })
}

/// The filter for tasks not yet done.
pub fn open_filter(done: &DoneBy) -> Value {
    match done {
        DoneBy::Checkbox(name) => json!({ "property": name, "checkbox": { "equals": false } }),
        DoneBy::Status { property, done } => {
            json!({ "property": property, "status": { "does_not_equal": done } })
        }
    }
}

/// The properties that mark a task done.
pub fn done_patch(done: &DoneBy) -> Value {
    match done {
        DoneBy::Checkbox(name) => json!({ "properties": { name: { "checkbox": true } } }),
        DoneBy::Status { property, done } => {
            json!({ "properties": { property: { "status": { "name": done } } } })
        }
    }
}

pub fn parse_tasks(results: &Value, title: &str) -> Vec<NotionTask> {
    results["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|page| {
            let text: String = page["properties"][title]["title"]
                .as_array()?
                .iter()
                .filter_map(|t| t["plain_text"].as_str())
                .collect();
            Some(NotionTask {
                id: page["id"].as_str()?.to_string(),
                title: if text.trim().is_empty() {
                    "Untitled".into()
                } else {
                    text
                },
                link: page["url"].as_str().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

fn token() -> AppResult<String> {
    key(KeyAccount::Notion).ok_or_else(|| {
        AppError::InvalidInput("Add your Notion integration token in Settings → Accounts.".into())
    })
}

fn database(settings_id: &str) -> AppResult<String> {
    database_id(settings_id).ok_or_else(|| {
        AppError::InvalidInput("Add the tasks database's link or ID in Settings → Accounts.".into())
    })
}

async fn call(request: reqwest::RequestBuilder, token: &str) -> AppResult<Value> {
    let response = request
        .bearer_auth(token)
        .header("Notion-Version", NOTION_VERSION)
        .send()
        .await
        .map_err(super::offline("Notion"))?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        if status == 404 {
            return Err(AppError::InvalidInput(
                "Notion can't see that database. Share it with your integration (••• → Connections).".into(),
            ));
        }
        return Err(problem("Notion", status));
    }
    response.json().await.map_err(super::offline("Notion"))
}

async fn schema_of(http: &reqwest::Client, token: &str, db: &str) -> AppResult<Schema> {
    let description = call(http.get(format!("{API}/databases/{db}")), token).await?;
    schema(&description).ok_or_else(|| {
        AppError::InvalidInput("That database has no checkbox or status property to mark tasks done.".into())
    })
}

pub async fn tasks(settings_id: &str) -> AppResult<Vec<NotionTask>> {
    let (token, db) = (token()?, database(settings_id)?);
    let http = super::http()?;
    let schema = schema_of(&http, &token, &db).await?;
    let body = json!({
        "filter": open_filter(&schema.done),
        "page_size": 50,
        "sorts": [{ "timestamp": "created_time", "direction": "ascending" }],
    });
    let results = call(
        http.post(format!("{API}/databases/{db}/query")).json(&body),
        &token,
    )
    .await?;
    Ok(parse_tasks(&results, &schema.title))
}

pub async fn complete(settings_id: &str, page_id: &str) -> AppResult<()> {
    if page_id.is_empty() || !page_id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(AppError::InvalidInput("not a task".into()));
    }
    let (token, db) = (token()?, database(settings_id)?);
    let http = super::http()?;
    let schema = schema_of(&http, &token, &db).await?;
    call(
        http.patch(format!("{API}/pages/{page_id}"))
            .json(&done_patch(&schema.done)),
        &token,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_ids_come_from_links_or_ids() {
        let id = "a8aec43384f447ed84390e8e42c2e089";
        assert_eq!(
            database_id(&format!("https://www.notion.so/team/Tasks-{id}?v=123")).as_deref(),
            Some(id)
        );
        assert_eq!(
            database_id("A8AEC433-84F4-47ED-8439-0E8E42C2E089").as_deref(),
            Some(id)
        );
        assert_eq!(database_id(id).as_deref(), Some(id));
        assert_eq!(database_id("tasks"), None);
    }

    #[test]
    fn a_checkbox_named_done_wins() {
        let db = json!({"properties": {
            "Name": {"type": "title"},
            "Urgent": {"type": "checkbox"},
            "Done": {"type": "checkbox"}}});
        let schema = schema(&db).unwrap();
        assert_eq!(schema.title, "Name");
        assert_eq!(schema.done, DoneBy::Checkbox("Done".into()));
        assert_eq!(
            open_filter(&schema.done),
            json!({"property": "Done", "checkbox": {"equals": false}})
        );
        assert_eq!(
            done_patch(&schema.done),
            json!({"properties": {"Done": {"checkbox": true}}})
        );
    }

    #[test]
    fn a_status_uses_its_complete_group() {
        let db = json!({"properties": {
            "Task": {"type": "title"},
            "Status": {"type": "status", "status": {
                "options": [{"id": "1", "name": "Not started"}, {"id": "2", "name": "Shipped"}],
                "groups": [{"name": "To-do", "option_ids": ["1"]}, {"name": "Complete", "option_ids": ["2"]}]}}}});
        let schema = schema(&db).unwrap();
        assert_eq!(
            schema.done,
            DoneBy::Status {
                property: "Status".into(),
                done: "Shipped".into()
            }
        );
        assert!(super::schema(&json!({"properties": {"Name": {"type": "title"}}})).is_none());
    }

    #[test]
    fn tasks_are_read_from_query_results() {
        let results = json!({"results": [
            {"id": "p-1", "url": "https://www.notion.so/p1",
             "properties": {"Name": {"title": [{"plain_text": "Write "}, {"plain_text": "report"}]}}},
            {"id": "p-2", "properties": {"Name": {"title": []}}}]});
        let tasks = parse_tasks(&results, "Name");
        assert_eq!(tasks[0].title, "Write report");
        assert_eq!(tasks[1].title, "Untitled");
    }
}
