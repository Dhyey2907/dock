//! Ask Claude: a question sent to the Messages API with the user's own key, the answer streamed
//! back to the popup as it is written. Use is billed to that key.

use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use super::{key, KeyAccount};
use crate::error::{AppError, AppResult};

pub const ASK_CLAUDE: &str = "ask-claude";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const MODEL: &str = "claude-sonnet-5-5";
const API_VERSION: &str = "2026-10-21";
/// Lets a refusal be retried on another model inside the same request.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const MAX_QUESTION: usize = 8000;
const SYSTEM: &str = "You answer questions asked from a small popup on a Windows desktop dock. \
Be direct and brief: lead with the answer, use short paragraphs, and use markdown only for \
short lists or code.";

/// The question being answered; asking another (or stopping) makes the older one stop.
static CURRENT: AtomicU32 = AtomicU32::new(0);

/// A piece of an answer, sent as it arrives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AskChunk {
    pub id: u32,
    /// Text to add to the answer.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The answer is complete (or stopped).
    pub done: bool,
    /// Why it stopped early, in words for people.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One server-sent event of the stream, as far as the popup cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Text(String),
    /// The reason the model stopped.
    Stop(String),
    Error(String),
    Other,
}

/// Read one `data:` payload of the stream.
pub fn parse_event(data: &str) -> StreamEvent {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
        return StreamEvent::Other;
    };
    match value["type"].as_str() {
        Some("content_block_delta") if value["delta"]["type"] == "text_delta" => {
            StreamEvent::Text(value["delta"]["text"].as_str().unwrap_or_default().to_string())
        }
        Some("message_delta") => match value["delta"]["stop_reason"].as_str() {
            Some(reason) => StreamEvent::Stop(reason.to_string()),
            None => StreamEvent::Other,
        },
        Some("error") => StreamEvent::Error(
            value["error"]["message"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
        ),
        _ => StreamEvent::Other,
    }
}

/// Split complete lines off `buffer`, leaving any partial line in it.
pub fn take_lines(buffer: &mut String) -> Vec<String> {
    let Some(end) = buffer.rfind('\n') else {
        return Vec::new();
    };
    let complete: String = buffer.drain(..=end).collect();
    complete
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect()
}

/// The request body for `question`.
pub fn body(question: &str) -> serde_json::Value {
    json!({
        "model": MODEL,
        "max_tokens": 16000,
        "stream": true,
        "system": SYSTEM,
        "output_config": { "effort": "low" },
        "fallbacks": "default",
        "messages": [{ "role": "user", "content": question }],
    })
}

fn http_problem(status: u16) -> String {
    match status {
        401 => "Your Anthropic API key wasn't accepted. Check it in Settings → Accounts.".into(),
        403 => "This API key isn't allowed to use Claude.".into(),
        429 => "Your API key hit its rate or spend limit. Try again shortly.".into(),
        500..=599 => "Claude is busy right now. Try again in a moment.".into(),
        code => format!("The Anthropic API answered with error {code}."),
    }
}

fn send(app: &AppHandle, chunk: AskChunk) {
    if let Err(error) = app.emit(ASK_CLAUDE, &chunk) {
        tracing::warn!(%error, "failed to send the answer");
    }
}

/// Stop the answer being written.
pub fn stop() {
    CURRENT.fetch_add(1, Ordering::SeqCst);
}

/// Ask `question`; the answer arrives as `ask-claude` events with the returned id.
pub fn ask(app: &AppHandle, question: &str) -> AppResult<u32> {
    let question: String = question.trim().chars().take(MAX_QUESTION).collect();
    if question.is_empty() {
        return Err(AppError::InvalidInput("Type a question first.".into()));
    }
    let api_key = key(KeyAccount::Anthropic)
        .ok_or_else(|| AppError::InvalidInput("Add your Anthropic API key in Settings → Accounts.".into()))?;
    let id = CURRENT.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let error = stream(&app, id, &api_key, &question).await.err();
        send(
            &app,
            AskChunk {
                id,
                text: None,
                done: true,
                error,
            },
        );
    });
    Ok(id)
}

async fn stream(app: &AppHandle, id: u32, api_key: &str, question: &str) -> Result<(), String> {
    let http = super::http().map_err(|_| "Couldn't start the request.".to_string())?;
    let mut response = http
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("anthropic-beta", FALLBACK_BETA)
        .header("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(300))
        .json(&body(question))
        .send()
        .await
        .map_err(|_| "Couldn't reach Claude. Check your connection.".to_string())?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        tracing::info!(status, "Claude request refused");
        return Err(http_problem(status));
    }
    let mut buffer = String::new();
    loop {
        if CURRENT.load(Ordering::SeqCst) != id {
            return Ok(());
        }
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => return Ok(()),
            Err(_) => return Err("The answer was cut off. Try again.".into()),
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        for line in take_lines(&mut buffer) {
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            match parse_event(data.trim()) {
                StreamEvent::Text(text) if !text.is_empty() => send(
                    app,
                    AskChunk {
                        id,
                        text: Some(text),
                        done: false,
                        error: None,
                    },
                ),
                StreamEvent::Stop(reason) if reason == "refusal" => {
                    return Err("Claude declined to answer that one.".into());
                }
                StreamEvent::Stop(reason) if reason == "max_tokens" => {
                    return Err("The answer was too long and got cut short.".into());
                }
                StreamEvent::Error(message) => {
                    tracing::info!(message, "Claude stream error");
                    return Err("Claude stopped with an error. Try again.".into());
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_events_are_read() {
        assert_eq!(
            parse_event(
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#
            ),
            StreamEvent::Text("Hi".into())
        );
        assert_eq!(
            parse_event(
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":""}}"#
            ),
            StreamEvent::Other
        );
        assert_eq!(
            parse_event(r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{}}"#),
            StreamEvent::Stop("end_turn".into())
        );
        assert_eq!(
            parse_event(r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#),
            StreamEvent::Error("Overloaded".into())
        );
        assert_eq!(parse_event("not json"), StreamEvent::Other);
    }

    #[test]
    fn lines_split_across_chunks_are_joined() {
        let mut buffer = String::from("event: a\ndata: {\"x\"");
        assert_eq!(take_lines(&mut buffer), vec!["event: a".to_string()]);
        buffer.push_str(":1}\r\n\r\n");
        assert_eq!(
            take_lines(&mut buffer),
            vec!["data: {\"x\":1}".to_string(), String::new()]
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn the_request_uses_the_chosen_model_and_streams() {
        let body = body("What is 2+2?");
        assert_eq!(body["model"], "claude-sonnet-5-5");
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["content"], "What is 2+2?");
        // Sampling settings are rejected by current models.
        assert!(body.get("temperature").is_none());
    }
}
