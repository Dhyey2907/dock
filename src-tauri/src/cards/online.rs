//! Public numbers for the card widgets, no account needed: stock quotes (Yahoo Finance's
//! chart feed), coin prices (CoinGecko), exchange rates (Frankfurter, the ECB's rates),
//! GitHub contributions and LeetCode counts by username. Each answer is kept for a few
//! minutes so the cards never hammer a service.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::error::{AppError, AppResult};

fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Dock")
        .build()
        .map_err(|e| AppError::Platform(e.to_string()))
}

static CACHE: Mutex<Option<HashMap<String, (Instant, Value)>>> = Mutex::new(None);

/// GET `url` as JSON, reusing an answer younger than `fresh`.
async fn cached_json(url: &str, fresh: Duration) -> AppResult<Value> {
    if let Some((at, value)) = CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .get(url)
    {
        if at.elapsed() < fresh {
            return Ok(value.clone());
        }
    }
    let value: Value = client()?
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| AppError::Platform(format!("couldn't reach the service: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Platform(format!("unexpected answer: {e}")))?;
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(url.to_string(), (Instant::now(), value.clone()));
    Ok(value)
}

fn encode(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || "-_.^=".contains(*c))
        .collect()
}

// ---------- Stocks ----------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub previous_close: f64,
    pub currency: String,
    /// Today's prices, oldest first (for the line).
    pub points: Vec<f64>,
}

pub fn parse_quote(symbol: &str, json: &Value) -> Option<Quote> {
    let result = json.get("chart")?.get("result")?.get(0)?;
    let meta = result.get("meta")?;
    let price = meta.get("regularMarketPrice")?.as_f64()?;
    let previous_close = meta
        .get("chartPreviousClose")
        .or_else(|| meta.get("previousClose"))
        .and_then(Value::as_f64)
        .unwrap_or(price);
    let points = result
        .get("indicators")
        .and_then(|i| i.get("quote"))
        .and_then(|q| q.get(0))
        .and_then(|q| q.get("close"))
        .and_then(Value::as_array)
        .map(|closes| closes.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    Some(Quote {
        symbol: symbol.to_string(),
        price,
        previous_close,
        currency: meta
            .get("currency")
            .and_then(Value::as_str)
            .unwrap_or("USD")
            .to_string(),
        points,
    })
}

/// Quotes for `symbols` (those that fail are left out).
pub async fn quotes(symbols: &[String]) -> Vec<Quote> {
    let mut out = Vec::new();
    for symbol in symbols.iter().take(3) {
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=5m&range=1d",
            encode(symbol)
        );
        if let Ok(json) = cached_json(&url, Duration::from_secs(120)).await {
            if let Some(quote) = parse_quote(symbol, &json) {
                out.push(quote);
            }
        }
    }
    out
}

// ---------- Coins ----------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoinPrice {
    pub id: String,
    pub usd: f64,
    /// Percent over the last 24 hours.
    pub change24h: f64,
}

pub async fn coin(id: &str) -> AppResult<CoinPrice> {
    let id = encode(id);
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={id}&vs_currencies=usd&include_24hr_change=true"
    );
    let json = cached_json(&url, Duration::from_secs(120)).await?;
    let entry = json
        .get(&id)
        .ok_or_else(|| AppError::InvalidInput("unknown coin".into()))?;
    Ok(CoinPrice {
        id: id.clone(),
        usd: entry.get("usd").and_then(Value::as_f64).unwrap_or(0.0),
        change24h: entry.get("usd_24h_change").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

// ---------- Currency ----------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Rate {
    pub from: String,
    pub to: String,
    pub rate: f64,
    /// The day the rate is from.
    pub date: String,
}

pub async fn rate(from: &str, to: &str) -> AppResult<Rate> {
    let (from, to) = (encode(from).to_uppercase(), encode(to).to_uppercase());
    if from == to {
        return Ok(Rate {
            from,
            to,
            rate: 1.0,
            date: String::new(),
        });
    }
    let url = format!("https://api.frankfurter.app/latest?from={from}&to={to}");
    let json = cached_json(&url, Duration::from_secs(3600)).await?;
    let rate = json
        .get("rates")
        .and_then(|r| r.get(&to))
        .and_then(Value::as_f64)
        .ok_or_else(|| AppError::InvalidInput("unknown currency".into()))?;
    Ok(Rate {
        from,
        to,
        rate,
        date: json.get("date").and_then(Value::as_str).unwrap_or("").to_string(),
    })
}

// ---------- GitHub contributions ----------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Contributions {
    pub user: String,
    /// The last 15 weeks, oldest first, seven days each (Sunday first): 0–4 intensity.
    pub levels: Vec<u8>,
    pub today: u32,
    pub week: u32,
    /// Days in a row with a contribution, up to today (or yesterday).
    pub streak: u32,
}

pub fn parse_contributions(user: &str, json: &Value, today: &str) -> Option<Contributions> {
    let days: Vec<(String, u32, u8)> = json
        .get("contributions")?
        .as_array()?
        .iter()
        .filter_map(|d| {
            Some((
                d.get("date")?.as_str()?.to_string(),
                d.get("count")?.as_u64()? as u32,
                d.get("level")?.as_u64()? as u8,
            ))
        })
        .filter(|(date, _, _)| date.as_str() <= today)
        .collect();
    let last = days.len().checked_sub(1)?;
    let count = |i: usize| days.get(i).map_or(0, |d| d.1);
    let today_count = count(last);
    let week = days.iter().rev().take(7).map(|d| d.1).sum();
    // A streak still counts if today has nothing yet.
    let mut i = if today_count == 0 {
        last.checked_sub(1)
    } else {
        Some(last)
    };
    let mut streak = 0;
    while let Some(at) = i {
        if count(at) == 0 {
            break;
        }
        streak += 1;
        i = at.checked_sub(1);
    }
    let shown = 15 * 7;
    let levels = days.iter().rev().take(shown).rev().map(|d| d.2).collect();
    Some(Contributions {
        user: user.to_string(),
        levels,
        today: today_count,
        week,
        streak,
    })
}

pub async fn contributions(user: &str) -> AppResult<Contributions> {
    if user.trim().is_empty() {
        return Err(AppError::InvalidInput("no GitHub username".into()));
    }
    let user = encode(user);
    let url = format!("https://github-contributions-api.jogruber.de/v4/{user}?y=last");
    let json = cached_json(&url, Duration::from_secs(900)).await?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    parse_contributions(&user, &json, &today)
        .ok_or_else(|| AppError::InvalidInput("no such GitHub user".into()))
}

// ---------- LeetCode ----------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LeetcodeStats {
    pub user: String,
    pub easy: u32,
    pub medium: u32,
    pub hard: u32,
    pub total: u32,
}

pub async fn leetcode(user: &str) -> AppResult<LeetcodeStats> {
    if user.trim().is_empty() {
        return Err(AppError::InvalidInput("no LeetCode username".into()));
    }
    let user = encode(user);
    let body = serde_json::json!({
        "query": "query($u:String!){matchedUser(username:$u){submitStatsGlobal{acSubmissionNum{difficulty count}}}}",
        "variables": { "u": user },
    });
    let json: Value = client()?
        .post("https://leetcode.com/graphql")
        .header("Referer", "https://leetcode.com")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Platform(format!("couldn't reach LeetCode: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Platform(format!("unexpected answer: {e}")))?;
    let counts = json
        .pointer("/data/matchedUser/submitStatsGlobal/acSubmissionNum")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::InvalidInput("no such LeetCode user".into()))?;
    let get = |name: &str| {
        counts
            .iter()
            .find(|c| c.get("difficulty").and_then(Value::as_str) == Some(name))
            .and_then(|c| c.get("count"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32
    };
    Ok(LeetcodeStats {
        user,
        easy: get("Easy"),
        medium: get("Medium"),
        hard: get("Hard"),
        total: get("All"),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_a_quote_and_its_line() {
        let json = json!({"chart":{"result":[{"meta":{"regularMarketPrice":190.5,"chartPreviousClose":188.0,"currency":"USD"},
            "indicators":{"quote":[{"close":[188.2,null,189.9,190.5]}]}}]}});
        let q = parse_quote("AAPL", &json).unwrap();
        assert_eq!(q.price, 190.5);
        assert_eq!(q.previous_close, 188.0);
        assert_eq!(q.points, vec![188.2, 189.9, 190.5]);
    }

    #[test]
    fn counts_contributions_and_a_streak_that_survives_an_empty_today() {
        let day = |d: &str, c: u32| json!({"date": d, "count": c, "level": if c > 0 { 2 } else { 0 }});
        let json = json!({"contributions": [
            day("2026-11-03", 0), day("2026-11-04", 2), day("2026-11-05", 1), day("2026-11-06", 3),
            day("2026-11-07", 0), day("2026-11-08", 9)
        ]});
        let c = parse_contributions("me", &json, "2026-11-07").unwrap();
        assert_eq!(c.today, 0);
        assert_eq!(c.streak, 3);
        assert_eq!(c.week, 6);
        assert_eq!(c.levels.len(), 5, "future days are left out");
    }

    #[test]
    fn keeps_only_url_safe_characters() {
        assert_eq!(encode("BRK.B / ../x"), "BRK.B..x");
        assert_eq!(encode("^NSEI"), "^NSEI");
    }
}
