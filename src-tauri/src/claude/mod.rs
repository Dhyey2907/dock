//! Claude usage, estimated from Claude Code's own transcripts on this PC (the desktop app and the
//! CLI both write them). Not your plan's official numbers: those aren't available to other apps.
//!
//! - Usage is counted per message (input, output and cache-write tokens; cache reads are left
//!   out, they're tiny in cost but huge in number).
//! - The 5-hour window starts at the first message after the last window ended, on the hour.
//! - When Claude stops you for a limit, the transcript says so and when it resets. The tokens used
//!   in that window become a sample of "your usual limit", which gives an estimated percentage.
//!
//! This file is the pure part; `service.rs` reads and watches the transcript files.

pub mod service;

use std::collections::HashMap;

use chrono::{DateTime, Duration, Local, Timelike, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const WINDOW: Duration = Duration::hours(5);
/// How many recent limit hits the "usual limit" is taken from.
const SAMPLES: usize = 3;

/// One assistant message's usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub id: String,
    pub at: DateTime<Utc>,
    pub model: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LimitKind {
    FiveHour,
    Weekly,
    Other,
}

/// Claude refused a message because a limit was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitHit {
    pub at: DateTime<Utc>,
    pub kind: LimitKind,
    pub resets_at: DateTime<Utc>,
}

#[derive(Debug, PartialEq)]
pub enum Line {
    Usage(Usage),
    Limit(LimitHit),
}

#[derive(Deserialize)]
struct RawLine {
    timestamp: Option<DateTime<Utc>>,
    message: Option<RawMessage>,
    #[serde(rename = "quotaLimits")]
    quota_limits: Option<RawQuota>,
}

#[derive(Deserialize)]
struct RawMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<RawUsage>,
}

#[derive(Deserialize, Default)]
struct RawUsage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    cache_creation_input_tokens: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawQuota {
    #[serde(default)]
    status: String,
    #[serde(default)]
    rate_limit_type: String,
    #[serde(default)]
    resets_at: i64,
}

/// What one transcript line tells us. Lines that can't matter are rejected by a substring check
/// before any JSON parsing, which keeps reading hundreds of MB quick.
pub fn parse_line(line: &str) -> Option<Line> {
    if !line.contains("\"type\":\"assistant\"") {
        return None;
    }
    let raw: RawLine = serde_json::from_str(line).ok()?;
    let at = raw.timestamp?;
    if let Some(quota) = raw
        .quota_limits
        .filter(|q| q.status == "rejected" && q.resets_at > 0)
    {
        let kind = match quota.rate_limit_type.as_str() {
            "five_hour" => LimitKind::FiveHour,
            "seven_day" | "seven_day_opus" | "seven_day_sonnet" => LimitKind::Weekly,
            _ => LimitKind::Other,
        };
        return Some(Line::Limit(LimitHit {
            at,
            kind,
            resets_at: DateTime::from_timestamp(quota.resets_at, 0)?,
        }));
    }
    let message = raw.message?;
    let model = message.model.filter(|m| !m.is_empty() && m != "<synthetic>")?;
    let usage = message.usage.unwrap_or_default();
    let tokens = (usage.input_tokens.max(0)
        + usage.output_tokens.max(0)
        + usage.cache_creation_input_tokens.max(0)) as u64;
    Some(Line::Usage(Usage {
        id: message.id?,
        at,
        model,
        tokens,
    }))
}

/// Start of the local hour containing `at`.
fn hour_floor(at: DateTime<Utc>) -> DateTime<Utc> {
    let local = at.with_timezone(&Local);
    let floored = local
        .with_minute(0)
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(local);
    floored.with_timezone(&Utc)
}

/// The 5-hour windows the messages fall into, oldest first, as (start, end).
pub fn windows(sorted: &[Usage]) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut out: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();
    for usage in sorted {
        if out.last().is_none_or(|(_, end)| usage.at >= *end) {
            let start = hour_floor(usage.at);
            out.push((start, start + WINDOW));
        }
    }
    out
}

fn tokens_between(sorted: &[Usage], from: DateTime<Utc>, to: DateTime<Utc>) -> u64 {
    sorted
        .iter()
        .filter(|u| u.at >= from && u.at < to)
        .map(|u| u.tokens)
        .sum()
}

/// "Your usual 5-hour limit": the median of the tokens used in the windows of the last few
/// times Claude stopped you. `None` until you've hit it once.
pub fn usual_limit(sorted: &[Usage], hits: &[LimitHit]) -> Option<u64> {
    let mut samples: Vec<u64> = hits
        .iter()
        .filter(|h| h.kind == LimitKind::FiveHour)
        .rev()
        .map(|h| tokens_between(sorted, h.resets_at - WINDOW, h.at + Duration::seconds(1)))
        .filter(|&t| t > 0)
        .take(SAMPLES)
        .collect();
    if samples.is_empty() {
        return None;
    }
    samples.sort_unstable();
    Some(samples[samples.len() / 2])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelTokens {
    pub model: String,
    #[ts(type = "number")]
    pub tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageWindow {
    #[ts(type = "string")]
    pub started_at: DateTime<Utc>,
    #[ts(type = "string")]
    pub resets_at: DateTime<Utc>,
    #[ts(type = "number")]
    pub tokens: u64,
    pub messages: u32,
    /// Share of your usual limit, when it's known (an estimate; can pass 100).
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u32>,
    /// Biggest first.
    pub models: Vec<ModelTokens>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActiveLimit {
    pub kind: LimitKind,
    #[ts(type = "string")]
    pub resets_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaudeUsage {
    /// The 5-hour window in progress; `None` when you haven't used Claude in the last 5 hours.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<UsageWindow>,
    /// A limit Claude is holding you to right now.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limited: Option<ActiveLimit>,
    #[ts(type = "number")]
    pub today_tokens: u64,
    /// The last seven days.
    #[ts(type = "number")]
    pub week_tokens: u64,
    #[ts(optional, type = "number")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usual_limit: Option<u64>,
    /// Whether any transcripts were found at all.
    pub found: bool,
}

/// Everything the widget shows, at `now`. `sorted` must be sorted by time and de-duplicated.
pub fn summarize(sorted: &[Usage], hits: &[LimitHit], now: DateTime<Utc>) -> ClaudeUsage {
    let usual = usual_limit(sorted, hits);
    let window = windows(sorted)
        .into_iter()
        .rev()
        .find(|(start, end)| *start <= now && now < *end)
        .map(|(start, end)| {
            let inside: Vec<&Usage> = sorted.iter().filter(|u| u.at >= start && u.at < end).collect();
            let tokens = inside.iter().map(|u| u.tokens).sum::<u64>();
            let mut by_model: HashMap<&str, u64> = HashMap::new();
            for u in &inside {
                *by_model.entry(u.model.as_str()).or_default() += u.tokens;
            }
            let mut models: Vec<ModelTokens> = by_model
                .into_iter()
                .map(|(model, tokens)| ModelTokens {
                    model: model.to_string(),
                    tokens,
                })
                .collect();
            models.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.model.cmp(&b.model)));
            UsageWindow {
                started_at: start,
                resets_at: end,
                tokens,
                messages: inside.len() as u32,
                percent: usual.map(|limit| ((tokens as f64 / limit as f64) * 100.0).round() as u32),
                models,
            }
        });
    let limited = hits
        .iter()
        .filter(|h| h.resets_at > now && h.at <= now)
        .max_by_key(|h| h.resets_at)
        .map(|h| ActiveLimit {
            kind: h.kind,
            resets_at: h.resets_at,
        });
    let midnight = now
        .with_timezone(&Local)
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|t| t.and_local_timezone(Local).earliest())
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or(now);
    ClaudeUsage {
        window,
        limited,
        today_tokens: tokens_between(sorted, midnight, now + Duration::seconds(1)),
        week_tokens: tokens_between(sorted, now - Duration::days(7), now + Duration::seconds(1)),
        usual_limit: usual,
        found: !sorted.is_empty() || !hits.is_empty(),
    }
}

/// When the summary next changes on its own (a window ends, a limit lifts), with no new messages.
pub fn next_change(usage: &ClaudeUsage, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    [
        usage.window.as_ref().map(|w| w.resets_at),
        usage.limited.map(|l| l.resets_at),
    ]
    .into_iter()
    .flatten()
    .filter(|t| *t > now)
    .min()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn usage(id: &str, at: &str, tokens: u64) -> Usage {
        Usage {
            id: id.into(),
            at: utc(at),
            model: "claude-opus-5-5".into(),
            tokens,
        }
    }

    #[test]
    fn reads_usage_and_limit_lines_and_skips_the_rest() {
        let assistant = r#"{"type":"assistant","timestamp":"2026-11-05T10:00:00Z","message":{"id":"msg_1","model":"claude-opus-5-5","usage":{"input_tokens":120,"output_tokens":800,"cache_creation_input_tokens":4000,"cache_read_input_tokens":90000}}}"#;
        assert_eq!(
            parse_line(assistant),
            Some(Line::Usage(Usage {
                id: "msg_1".into(),
                at: utc("2026-11-05T10:00:00Z"),
                model: "claude-opus-5-5".into(),
                tokens: 4920,
            }))
        );
        let limit = r#"{"type":"assistant","timestamp":"2026-09-16T15:43:56.776Z","message":{"model":"<synthetic>","usage":{"input_tokens":0}},"quotaLimits":{"status":"rejected","resetsAt":1789583400,"rateLimitType":"five_hour"},"isApiErrorMessage":true}"#;
        assert_eq!(
            parse_line(limit),
            Some(Line::Limit(LimitHit {
                at: utc("2026-09-16T15:43:56.776Z"),
                kind: LimitKind::FiveHour,
                resets_at: DateTime::from_timestamp(1_789_583_400, 0).unwrap(),
            }))
        );
        assert_eq!(parse_line(r#"{"type":"user","message":{"content":"hi"}}"#), None);
        assert_eq!(
            parse_line(r#"{"type":"assistant","message":{"model":"<synthetic>"}}"#),
            None
        );
        assert_eq!(parse_line("not json \"type\":\"assistant\""), None);
    }

    #[test]
    fn windows_start_on_the_hour_and_run_five_hours() {
        let list = [
            usage("a", "2026-11-05T09:40:00Z", 1),
            usage("b", "2026-11-05T12:00:00Z", 1),
            // After the first window ends: a new one.
            usage("c", "2026-11-05T15:10:00Z", 1),
        ];
        let w = windows(&list);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].1 - w[0].0, WINDOW);
        assert!(w[0].0 <= list[0].at && list[1].at < w[0].1);
        assert!(w[1].0 <= list[2].at);
    }

    #[test]
    fn the_usual_limit_is_learned_from_past_limit_hits() {
        let list = [
            usage("a", "2026-11-01T10:10:00Z", 600_000),
            usage("b", "2026-11-01T12:00:00Z", 400_000),
            usage("c", "2026-11-03T10:30:00Z", 800_000),
            usage("d", "2026-11-04T09:00:00Z", 1_200_000),
        ];
        let hit = |at: &str, resets: &str| LimitHit {
            at: utc(at),
            kind: LimitKind::FiveHour,
            resets_at: utc(resets),
        };
        let hits = [
            hit("2026-11-01T12:30:00Z", "2026-11-01T15:00:00Z"), // 1.0M in that window
            hit("2026-11-03T11:00:00Z", "2026-11-03T15:00:00Z"), // 0.8M
            hit("2026-11-04T10:00:00Z", "2026-11-04T13:00:00Z"), // 1.2M
        ];
        assert_eq!(usual_limit(&list, &hits), Some(1_000_000));
        assert_eq!(usual_limit(&list, &[]), None);
    }

    #[test]
    fn summary_shows_the_current_window_and_any_active_limit() {
        let list = [
            usage("old", "2026-11-04T08:00:00Z", 5_000),
            usage("a", "2026-11-05T09:10:00Z", 300_000),
            usage("b", "2026-11-05T10:00:00Z", 200_000),
        ];
        let hits = [LimitHit {
            at: utc("2026-11-02T10:00:00Z"),
            kind: LimitKind::FiveHour,
            resets_at: utc("2026-11-02T12:00:00Z"),
        }];
        // A past window to learn from: 1M tokens.
        let mut all = vec![usage("p", "2026-11-02T08:00:00Z", 1_000_000)];
        all.extend(list.iter().cloned());
        let now = utc("2026-11-05T10:30:00Z");
        let summary = summarize(&all, &hits, now);
        let window = summary.window.clone().unwrap();
        assert_eq!(window.tokens, 500_000);
        assert_eq!(window.messages, 2);
        assert_eq!(window.percent, Some(50));
        assert_eq!(window.models[0].tokens, 500_000);
        assert_eq!(summary.week_tokens, 1_505_000);
        assert!(summary.limited.is_none());
        assert_eq!(next_change(&summary, now), Some(window.resets_at));
    }

    #[test]
    fn a_limit_in_force_is_reported_until_it_resets() {
        let hits = [LimitHit {
            at: utc("2026-11-05T10:00:00Z"),
            kind: LimitKind::Weekly,
            resets_at: utc("2026-11-07T09:00:00Z"),
        }];
        let during = summarize(&[], &hits, utc("2026-11-05T11:00:00Z"));
        assert_eq!(during.limited.map(|l| l.kind), Some(LimitKind::Weekly));
        assert!(summarize(&[], &hits, utc("2026-11-07T09:00:01Z"))
            .limited
            .is_none());
    }

    #[test]
    fn no_window_after_five_quiet_hours() {
        let list = [usage("a", "2026-11-05T01:00:00Z", 10)];
        assert!(summarize(&list, &[], utc("2026-11-05T08:00:00Z"))
            .window
            .is_none());
    }
}
