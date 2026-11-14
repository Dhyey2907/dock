//! The account card widgets (Lumabar's paid-service set): revenue from Stripe, Polar, Shopify,
//! Gumroad, Paddle, Lemon Squeezy and Dodo Payments; traffic from Plausible, DataFast and Dub;
//! Resend, Linear and Supabase; YouTube, Instagram and X followers; a DeepSeek balance; GitHub
//! notifications; and Gmail's unread count.
//!
//! Each service's key is the user's own, read-only where the service allows it, and lives in
//! Windows Credential Manager. Answers are kept for ten minutes so the cards never hammer an API.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use chrono::{DateTime, Duration as Days, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::{http, offline, problem, secret_delete, secret_get, secret_set};
use crate::{
    error::{AppError, AppResult},
    settings::SettingsStore,
};

/// A service whose key the user pastes in for a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Service {
    Stripe,
    Polar,
    Shopify,
    Gumroad,
    Paddle,
    LemonSqueezy,
    Dodo,
    Plausible,
    Datafast,
    Dub,
    Resend,
    Linear,
    Supabase,
    Youtube,
    Instagram,
    X,
    Deepseek,
}

impl Service {
    pub const ALL: [Service; 17] = [
        Service::Stripe,
        Service::Polar,
        Service::Shopify,
        Service::Gumroad,
        Service::Paddle,
        Service::LemonSqueezy,
        Service::Dodo,
        Service::Plausible,
        Service::Datafast,
        Service::Dub,
        Service::Resend,
        Service::Linear,
        Service::Supabase,
        Service::Youtube,
        Service::Instagram,
        Service::X,
        Service::Deepseek,
    ];

    fn secret_name(self) -> String {
        format!(
            "card-{}",
            serde_json::to_value(self)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        )
    }

    fn label(self) -> &'static str {
        match self {
            Service::Stripe => "Stripe",
            Service::Polar => "Polar",
            Service::Shopify => "Shopify",
            Service::Gumroad => "Gumroad",
            Service::Paddle => "Paddle",
            Service::LemonSqueezy => "Lemon Squeezy",
            Service::Dodo => "Dodo Payments",
            Service::Plausible => "Plausible",
            Service::Datafast => "DataFast",
            Service::Dub => "Dub",
            Service::Resend => "Resend",
            Service::Linear => "Linear",
            Service::Supabase => "Supabase",
            Service::Youtube => "YouTube",
            Service::Instagram => "Instagram",
            Service::X => "X",
            Service::Deepseek => "DeepSeek",
        }
    }

    /// The services that report revenue.
    pub const REVENUE: [Service; 7] = [
        Service::Stripe,
        Service::Polar,
        Service::Shopify,
        Service::Gumroad,
        Service::Paddle,
        Service::LemonSqueezy,
        Service::Dodo,
    ];
}

fn key_of(service: Service) -> AppResult<String> {
    secret_get(&service.secret_name()).ok_or_else(|| {
        AppError::InvalidInput(format!("Add your {} key in this card's popup.", service.label()))
    })
}

/// Save (or, empty, remove) a service's key. Returns it trimmed.
pub fn set_key(service: Service, value: &str) -> AppResult<()> {
    let value = value.trim();
    if value.is_empty() {
        secret_delete(&service.secret_name());
        clear_cache();
        return Ok(());
    }
    if value.len() > 600 || value.chars().any(char::is_whitespace) {
        return Err(AppError::InvalidInput(
            "That doesn't look like a key. Paste just the key.".into(),
        ));
    }
    let check = |prefixes: &[&str], hint: &str| -> AppResult<()> {
        if prefixes.iter().any(|p| value.starts_with(p)) {
            Ok(())
        } else {
            Err(AppError::InvalidInput(hint.into()))
        }
    };
    match service {
        Service::Stripe => check(
            &["rk_", "sk_"],
            "Stripe keys start with \"rk_\" (a restricted, read-only key is best) or \"sk_\".",
        )?,
        Service::Polar => check(&["polar_"], "Polar access tokens start with \"polar_\".")?,
        Service::Shopify => check(
            &["shpat_", "shpca_"],
            "Shopify Admin API tokens start with \"shpat_\".",
        )?,
        Service::Datafast => check(&["df_"], "DataFast API keys start with \"df_\".")?,
        Service::Dub => check(&["dub_"], "Dub API keys start with \"dub_\".")?,
        Service::Resend => check(&["re_"], "Resend API keys start with \"re_\".")?,
        Service::Linear => check(&["lin_api_"], "Linear personal API keys start with \"lin_api_\".")?,
        Service::Supabase => check(&["sbp_"], "Supabase access tokens start with \"sbp_\".")?,
        Service::Deepseek => check(&["sk-"], "DeepSeek API keys start with \"sk-\".")?,
        _ if value.len() < 8 => return Err(AppError::InvalidInput("That key is too short.".into())),
        _ => {}
    }
    secret_set(&service.secret_name(), value)?;
    clear_cache();
    Ok(())
}

/// Which services have a key saved (never the keys themselves).
pub fn connected() -> Vec<Service> {
    Service::ALL
        .into_iter()
        .filter(|s| secret_get(&s.secret_name()).is_some())
        .collect()
}

// ---------- Fetching, with a short memory ----------

static CACHE: Mutex<Option<HashMap<String, (Instant, String)>>> = Mutex::new(None);
const FRESH: Duration = Duration::from_secs(600);

fn clear_cache() {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn cached<T: Clone + Serialize + for<'de> Deserialize<'de>>(key: &str) -> Option<T> {
    let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let (at, text) = cache.as_ref()?.get(key)?;
    (at.elapsed() < FRESH)
        .then(|| serde_json::from_str(text).ok())
        .flatten()
}

fn remember<T: Serialize>(key: &str, value: &T) {
    if let Ok(text) = serde_json::to_string(value) {
        CACHE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(HashMap::new)
            .insert(key.to_string(), (Instant::now(), text));
    }
}

/// Answer a card's question from memory, or ask `fetch` and remember the answer.
async fn memo<T, F>(key: &str, fetch: F) -> AppResult<T>
where
    T: Clone + Serialize + for<'de> Deserialize<'de>,
    F: std::future::Future<Output = AppResult<T>>,
{
    if let Some(hit) = cached::<T>(key) {
        return Ok(hit);
    }
    let value = fetch.await?;
    remember(key, &value);
    Ok(value)
}

/// Send a request and read its JSON, turning failures into sentences for the card.
async fn json(service: Service, request: reqwest::RequestBuilder) -> AppResult<Value> {
    let name = service.label();
    let response = request.send().await.map_err(offline(name))?;
    let status = response.status();
    if !status.is_success() {
        return Err(problem(name, status.as_u16()));
    }
    response
        .json()
        .await
        .map_err(|_| AppError::InvalidInput(format!("{name} sent an answer Dock couldn't read.")))
}

// ---------- Revenue ----------

/// Revenue for a card: the last 30 days, day by day (oldest first), in major units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Revenue {
    pub service: Service,
    /// "USD".
    pub currency: String,
    /// The last 30 days, today last.
    pub daily: Vec<f64>,
    pub total: f64,
}

const DAYS: i64 = 30;

/// Spread (when, amount) pairs over the last 30 days (today last), in the given currency.
pub fn bucket(
    service: Service,
    currency: &str,
    items: &[(DateTime<Utc>, f64)],
    now: DateTime<Utc>,
) -> Revenue {
    let today = now.date_naive();
    let first = today - Days::days(DAYS - 1);
    let mut daily = vec![0.0; DAYS as usize];
    for (at, amount) in items {
        let day = at.date_naive();
        if day < first || day > today {
            continue;
        }
        daily[(day - first).num_days() as usize] += amount;
    }
    let total = daily.iter().sum();
    Revenue {
        service,
        currency: currency.to_uppercase(),
        daily,
        total,
    }
}

fn since(now: DateTime<Utc>) -> DateTime<Utc> {
    now - Days::days(DAYS)
}

fn parse_time(v: &Value) -> Option<DateTime<Utc>> {
    match v {
        Value::Number(n) => DateTime::from_timestamp(n.as_i64()?, 0),
        Value::String(s) => DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .ok()
            .or_else(|| {
                NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(12, 0, 0))
                    .map(|d| d.and_utc())
            }),
        _ => None,
    }
}

fn num(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Stripe: the net of every balance movement except payouts (what came in, less fees and
/// refunds), from the balance transactions.
async fn stripe(key: &str, now: DateTime<Utc>) -> AppResult<Revenue> {
    let client = http()?;
    let mut items = Vec::new();
    let mut currency = String::from("usd");
    let mut after: Option<String> = None;
    for _ in 0..10 {
        let mut request = client
            .get("https://api.stripe.com/v1/balance_transactions")
            .bearer_auth(key)
            .query(&[
                ("limit", "100"),
                ("created[gte]", &since(now).timestamp().to_string()),
            ]);
        if let Some(id) = &after {
            request = request.query(&[("starting_after", id.as_str())]);
        }
        let page = json(Service::Stripe, request).await?;
        let data = page
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for t in &data {
            let kind = t.get("type").and_then(Value::as_str).unwrap_or("");
            if kind.contains("payout") || kind.contains("transfer") {
                continue;
            }
            if let Some(c) = t.get("currency").and_then(Value::as_str) {
                currency = c.to_string();
            }
            if let Some(at) = t.get("created").and_then(parse_time) {
                items.push((at, num(t.get("net")) / 100.0));
            }
        }
        if page.get("has_more").and_then(Value::as_bool) != Some(true) {
            break;
        }
        after = data
            .last()
            .and_then(|t| t.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    Ok(bucket(Service::Stripe, &currency, &items, now))
}

/// Polar: net revenue per day, from its metrics.
async fn polar(key: &str, now: DateTime<Utc>) -> AppResult<Revenue> {
    let start = since(now).format("%Y-%m-%d").to_string();
    let end = now.format("%Y-%m-%d").to_string();
    let answer = json(
        Service::Polar,
        http()?
            .get("https://api.polar.sh/v1/metrics/")
            .bearer_auth(key)
            .query(&[
                ("start_date", start.as_str()),
                ("end_date", end.as_str()),
                ("interval", "day"),
                ("timezone", "UTC"),
            ]),
    )
    .await?;
    let items: Vec<_> = answer
        .get("periods")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            Some((
                parse_time(p.get("timestamp")?)?,
                num(p.get("net_revenue")) / 100.0,
            ))
        })
        .collect();
    Ok(bucket(Service::Polar, "USD", &items, now))
}

/// Shopify: each order's total, less refunds, from the Admin API.
async fn shopify(key: &str, store: &str, now: DateTime<Utc>) -> AppResult<Revenue> {
    let store = store.trim().trim_start_matches("https://").trim_end_matches('/');
    if store.is_empty() {
        return Err(AppError::InvalidInput(
            "Add your store's address (name.myshopify.com) in this card's popup.".into(),
        ));
    }
    let host = if store.contains('.') {
        store.to_string()
    } else {
        format!("{store}.myshopify.com")
    };
    let answer = json(
        Service::Shopify,
        http()?
            .get(format!("https://{host}/admin/api/2025-01/orders.json"))
            .header("X-Shopify-Access-Token", key)
            .query(&[
                ("status", "any"),
                ("limit", "250"),
                ("created_at_min", &since(now).to_rfc3339()),
                (
                    "fields",
                    "created_at,current_total_price,currency,financial_status",
                ),
            ]),
    )
    .await?;
    let orders = answer
        .get("orders")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let currency = orders
        .first()
        .and_then(|o| o.get("currency"))
        .and_then(Value::as_str)
        .unwrap_or("USD")
        .to_string();
    let items: Vec<_> = orders
        .iter()
        .filter(|o| o.get("financial_status").and_then(Value::as_str) != Some("voided"))
        .filter_map(|o| {
            Some((
                parse_time(o.get("created_at")?)?,
                num(o.get("current_total_price")),
            ))
        })
        .collect();
    Ok(bucket(Service::Shopify, &currency, &items, now))
}

/// Gumroad: what each sale left you after Gumroad's fee.
async fn gumroad(key: &str, now: DateTime<Utc>) -> AppResult<Revenue> {
    let client = http()?;
    let after = since(now).format("%Y-%m-%d").to_string();
    let mut items = Vec::new();
    let mut page_key: Option<String> = None;
    for _ in 0..10 {
        let mut request = client
            .get("https://api.gumroad.com/v2/sales")
            .query(&[("access_token", key), ("after", after.as_str())]);
        if let Some(k) = &page_key {
            request = request.query(&[("page_key", k.as_str())]);
        }
        let page = json(Service::Gumroad, request).await?;
        for sale in page.get("sales").and_then(Value::as_array).into_iter().flatten() {
            if sale.get("refunded").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let kept = num(sale.get("price")) - num(sale.get("gumroad_fee"));
            if let Some(at) = sale.get("created_at").and_then(parse_time) {
                items.push((at, kept / 100.0));
            }
        }
        page_key = page
            .get("next_page_key")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if page_key.is_none() {
            break;
        }
    }
    Ok(bucket(Service::Gumroad, "USD", &items, now))
}

/// Paddle: your earnings from each completed transaction.
async fn paddle(key: &str, now: DateTime<Utc>) -> AppResult<Revenue> {
    let client = http()?;
    let mut items = Vec::new();
    let mut currency = String::from("USD");
    let mut url = Some(format!(
        "https://api.paddle.com/transactions?status=completed,billed&per_page=200&created_at[GTE]={}",
        since(now).format("%Y-%m-%dT%H:%M:%SZ")
    ));
    for _ in 0..10 {
        let Some(next) = url.take() else { break };
        let page = json(Service::Paddle, client.get(&next).bearer_auth(key)).await?;
        for t in page.get("data").and_then(Value::as_array).into_iter().flatten() {
            let totals = t.pointer("/details/totals");
            if let Some(c) = totals
                .and_then(|x| x.get("currency_code"))
                .and_then(Value::as_str)
            {
                currency = c.to_string();
            }
            let earned = num(totals.and_then(|x| x.get("earnings")));
            if let Some(at) = t.get("created_at").and_then(parse_time) {
                items.push((at, earned / 100.0));
            }
        }
        let more = page.pointer("/meta/pagination/has_more").and_then(Value::as_bool) == Some(true);
        url = more
            .then(|| {
                page.pointer("/meta/pagination/next")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .flatten();
    }
    Ok(bucket(Service::Paddle, &currency, &items, now))
}

/// Lemon Squeezy: each paid order's total less tax.
async fn lemon(key: &str, now: DateTime<Utc>) -> AppResult<Revenue> {
    let client = http()?;
    let mut items = Vec::new();
    let mut currency = String::from("USD");
    let mut url = Some("https://api.lemonsqueezy.com/v1/orders?page[size]=100&sort=-created_at".to_string());
    'pages: for _ in 0..10 {
        let Some(next) = url.take() else { break };
        let page = json(
            Service::LemonSqueezy,
            client
                .get(&next)
                .bearer_auth(key)
                .header("Accept", "application/vnd.api+json"),
        )
        .await?;
        for order in page.get("data").and_then(Value::as_array).into_iter().flatten() {
            let a = order.get("attributes");
            let Some(at) = a.and_then(|a| a.get("created_at")).and_then(parse_time) else {
                continue;
            };
            if at < since(now) {
                break 'pages;
            }
            if a.and_then(|a| a.get("status")).and_then(Value::as_str) != Some("paid") {
                continue;
            }
            if let Some(c) = a.and_then(|a| a.get("currency")).and_then(Value::as_str) {
                currency = c.to_string();
            }
            let kept = num(a.and_then(|a| a.get("total"))) - num(a.and_then(|a| a.get("tax")));
            items.push((at, kept / 100.0));
        }
        url = page
            .pointer("/links/next")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    Ok(bucket(Service::LemonSqueezy, &currency, &items, now))
}

/// Dodo Payments: each succeeded payment's total.
async fn dodo(key: &str, now: DateTime<Utc>) -> AppResult<Revenue> {
    let client = http()?;
    let mut items = Vec::new();
    let mut currency = String::from("USD");
    for page_number in 0..10 {
        let page = json(
            Service::Dodo,
            client
                .get("https://live.dodopayments.com/payments")
                .bearer_auth(key)
                .query(&[
                    ("created_at_gte", since(now).to_rfc3339().as_str()),
                    ("status", "succeeded"),
                    ("page_size", "100"),
                    ("page_number", &page_number.to_string()),
                ]),
        )
        .await?;
        let list = page
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for p in &list {
            if let Some(c) = p.get("currency").and_then(Value::as_str) {
                currency = c.to_string();
            }
            if let Some(at) = p.get("created_at").and_then(parse_time) {
                items.push((at, num(p.get("total_amount")) / 100.0));
            }
        }
        if list.len() < 100 {
            break;
        }
    }
    Ok(bucket(Service::Dodo, &currency, &items, now))
}

/// The last 30 days of revenue from `service`.
pub async fn revenue(app: &AppHandle, service: Service) -> AppResult<Revenue> {
    if !Service::REVENUE.contains(&service) {
        return Err(AppError::InvalidInput("not a revenue service".into()));
    }
    let key = key_of(service)?;
    let store = app.state::<SettingsStore>().get().cards.shopify_store;
    let now = Utc::now();
    memo(&format!("revenue/{service:?}"), async move {
        match service {
            Service::Stripe => stripe(&key, now).await,
            Service::Polar => polar(&key, now).await,
            Service::Shopify => shopify(&key, &store, now).await,
            Service::Gumroad => gumroad(&key, now).await,
            Service::Paddle => paddle(&key, now).await,
            Service::LemonSqueezy => lemon(&key, now).await,
            _ => dodo(&key, now).await,
        }
    })
    .await
}

/// Every connected store's revenue, for the Total Revenue card (each in its own currency;
/// the page converts).
pub async fn all_revenue(app: &AppHandle) -> Vec<Revenue> {
    let mut out = Vec::new();
    for service in Service::REVENUE {
        if secret_get(&service.secret_name()).is_none() {
            continue;
        }
        if let Ok(r) = revenue(app, service).await {
            out.push(r);
        }
    }
    out
}

// ---------- Analytics ----------

/// Visitors or clicks: this week, the week before, and the last 14 days.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Traffic {
    pub service: Service,
    pub week: f64,
    pub previous: f64,
    /// The last 14 days, today last.
    pub daily: Vec<f64>,
}

fn traffic_from_daily(service: Service, daily: Vec<f64>) -> Traffic {
    let n = daily.len();
    let week = daily.iter().skip(n.saturating_sub(7)).sum();
    let previous = daily
        .iter()
        .skip(n.saturating_sub(14))
        .take(7.min(n.saturating_sub(7)))
        .sum();
    Traffic {
        service,
        week,
        previous,
        daily,
    }
}

/// Plausible: visitors per day for the last 14 days (Stats API v2).
async fn plausible(key: &str, site: &str) -> AppResult<Traffic> {
    if site.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "Add your site's domain in this card's popup.".into(),
        ));
    }
    let body = serde_json::json!({
        "site_id": site.trim(),
        "metrics": ["visitors"],
        "date_range": "14d",
        "dimensions": ["time:day"],
    });
    let answer = json(
        Service::Plausible,
        http()?
            .post("https://plausible.io/api/v2/query")
            .bearer_auth(key)
            .json(&body),
    )
    .await?;
    let mut by_day: HashMap<String, f64> = HashMap::new();
    for row in answer
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let day = row.pointer("/dimensions/0").and_then(Value::as_str).unwrap_or("");
        by_day.insert(
            day.get(..10).unwrap_or("").to_string(),
            num(row.pointer("/metrics/0")),
        );
    }
    Ok(traffic_from_daily(Service::Plausible, last_days(14, &by_day)))
}

/// The last `n` days from a "YYYY-MM-DD" -> value map, oldest first.
fn last_days(n: i64, by_day: &HashMap<String, f64>) -> Vec<f64> {
    let today = Utc::now().date_naive();
    (0..n)
        .rev()
        .map(|back| {
            let day = (today - Days::days(back)).format("%Y-%m-%d").to_string();
            by_day.get(&day).copied().unwrap_or(0.0)
        })
        .collect()
}

/// DataFast: visitors per day for the last 14 days.
async fn datafast(key: &str) -> AppResult<Traffic> {
    let today = Utc::now().date_naive();
    let start = (today - Days::days(13)).format("%Y-%m-%d").to_string();
    let end = today.format("%Y-%m-%d").to_string();
    let answer = json(
        Service::Datafast,
        http()?
            .get("https://datafa.st/api/v1/analytics/timeseries")
            .bearer_auth(key)
            .query(&[
                ("fields", "visitors"),
                ("interval", "day"),
                ("startAt", start.as_str()),
                ("endAt", end.as_str()),
            ]),
    )
    .await?;
    let mut by_day = HashMap::new();
    for row in answer.get("data").and_then(Value::as_array).into_iter().flatten() {
        let day = ["timestamp", "date", "name", "time"]
            .iter()
            .find_map(|k| row.get(*k).and_then(Value::as_str))
            .unwrap_or("");
        by_day.insert(day.get(..10).unwrap_or("").to_string(), num(row.get("visitors")));
    }
    Ok(traffic_from_daily(Service::Datafast, last_days(14, &by_day)))
}

/// Dub: clicks on your links per day for the last 14 days.
async fn dub(key: &str) -> AppResult<Traffic> {
    let answer = json(
        Service::Dub,
        http()?
            .get("https://api.dub.co/analytics")
            .bearer_auth(key)
            .query(&[
                ("event", "clicks"),
                ("interval", "30d"),
                ("groupBy", "timeseries"),
            ]),
    )
    .await?;
    let mut by_day = HashMap::new();
    for row in answer.as_array().into_iter().flatten() {
        let day = row.get("start").and_then(Value::as_str).unwrap_or("");
        *by_day
            .entry(day.get(..10).unwrap_or("").to_string())
            .or_insert(0.0) += num(row.get("clicks"));
    }
    Ok(traffic_from_daily(Service::Dub, last_days(14, &by_day)))
}

pub async fn traffic(app: &AppHandle, service: Service) -> AppResult<Traffic> {
    let key = key_of(service)?;
    let site = app.state::<SettingsStore>().get().cards.plausible_site;
    memo(&format!("traffic/{service:?}"), async move {
        match service {
            Service::Plausible => plausible(&key, &site).await,
            Service::Datafast => datafast(&key).await,
            Service::Dub => dub(&key).await,
            _ => Err(AppError::InvalidInput("not an analytics service".into())),
        }
    })
    .await
}

// ---------- Developer services ----------

/// A number or two for the developer and social cards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Count {
    pub service: Service,
    /// The headline number (emails sent, issues assigned, followers…).
    pub value: f64,
    /// A second number when there is one (delivered, in progress, requests…).
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second: Option<f64>,
    /// Words for the card ("Healthy", the account's name, the currency…).
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Lines to list in the popup (issue titles…).
    pub items: Vec<String>,
}

fn count(service: Service, value: f64) -> Count {
    Count {
        service,
        value,
        second: None,
        note: None,
        items: Vec::new(),
    }
}

/// Resend: emails sent in the last seven days, and how many were delivered.
async fn resend(key: &str) -> AppResult<Count> {
    let answer = json(
        Service::Resend,
        http()?
            .get("https://api.resend.com/emails")
            .bearer_auth(key)
            .query(&[("limit", "100")]),
    )
    .await?;
    let week_ago = Utc::now() - Days::days(7);
    let recent: Vec<&Value> = answer
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|e| {
            e.get("created_at")
                .and_then(parse_time)
                .is_some_and(|t| t >= week_ago)
        })
        .collect();
    let delivered = recent
        .iter()
        .filter(|e| e.get("last_event").and_then(Value::as_str) == Some("delivered"))
        .count();
    Ok(Count {
        second: Some(delivered as f64),
        ..count(Service::Resend, recent.len() as f64)
    })
}

/// Linear: issues assigned to you that aren't done, and how many are in progress.
async fn linear(key: &str) -> AppResult<Count> {
    let query = r#"{ viewer { assignedIssues(first: 50, filter: { state: { type: { nin: ["completed", "canceled"] } } }) { nodes { identifier title state { type } } } } }"#;
    let answer = json(
        Service::Linear,
        http()?
            .post("https://api.linear.app/graphql")
            .header("Authorization", key)
            .json(&serde_json::json!({ "query": query })),
    )
    .await?;
    let nodes = answer
        .pointer("/data/viewer/assignedIssues/nodes")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AppError::InvalidInput("Linear didn't accept the key.".into()))?;
    let started = nodes
        .iter()
        .filter(|n| n.pointer("/state/type").and_then(Value::as_str) == Some("started"))
        .count();
    let items = nodes
        .iter()
        .take(12)
        .map(|n| {
            format!(
                "{} {}",
                n.get("identifier").and_then(Value::as_str).unwrap_or(""),
                n.get("title").and_then(Value::as_str).unwrap_or("")
            )
        })
        .collect();
    Ok(Count {
        second: Some(started as f64),
        items,
        ..count(Service::Linear, nodes.len() as f64)
    })
}

/// Supabase: API requests on the project over the last day, and whether it's healthy.
async fn supabase(key: &str, project: &str) -> AppResult<Count> {
    let project = project.trim();
    if project.is_empty() {
        return Err(AppError::InvalidInput(
            "Add your project's reference ID in this card's popup.".into(),
        ));
    }
    let client = http()?;
    let usage = json(
        Service::Supabase,
        client
            .get(format!(
                "https://api.supabase.com/v1/projects/{project}/analytics/endpoints/usage.api-counts"
            ))
            .bearer_auth(key)
            .query(&[("interval", "1day")]),
    )
    .await?;
    let requests: f64 = usage
        .get("result")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|row| {
            [
                "total_rest_requests",
                "total_auth_requests",
                "total_storage_requests",
                "total_realtime_requests",
            ]
            .iter()
            .map(|k| num(row.get(*k)))
            .sum::<f64>()
        })
        .sum();
    let health = json(
        Service::Supabase,
        client
            .get(format!("https://api.supabase.com/v1/projects/{project}/health"))
            .bearer_auth(key)
            .query(&[("services", "db"), ("services", "auth"), ("services", "rest")]),
    )
    .await
    .ok();
    let healthy = health.as_ref().and_then(Value::as_array).is_some_and(|list| {
        list.iter()
            .all(|s| s.get("healthy").and_then(Value::as_bool) == Some(true))
    });
    Ok(Count {
        note: Some(
            if health.is_none() {
                "Unknown"
            } else if healthy {
                "Healthy"
            } else {
                "Trouble"
            }
            .into(),
        ),
        ..count(Service::Supabase, requests)
    })
}

/// YouTube: subscribers on a channel (Data API key plus the channel's ID).
async fn youtube(key: &str, channel: &str) -> AppResult<Count> {
    if channel.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "Add your channel ID (UC…) in this card's popup.".into(),
        ));
    }
    let answer = json(
        Service::Youtube,
        http()?
            .get("https://www.googleapis.com/youtube/v3/channels")
            .query(&[
                ("part", "statistics,snippet"),
                ("id", channel.trim()),
                ("key", key),
            ]),
    )
    .await?;
    let item = answer
        .pointer("/items/0")
        .ok_or_else(|| AppError::InvalidInput("YouTube found no such channel.".into()))?;
    Ok(Count {
        second: Some(num(item.pointer("/statistics/viewCount"))),
        note: item
            .pointer("/snippet/title")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..count(Service::Youtube, num(item.pointer("/statistics/subscriberCount")))
    })
}

/// Instagram: followers, with an Instagram API access token.
async fn instagram(key: &str) -> AppResult<Count> {
    let answer = json(
        Service::Instagram,
        http()?.get("https://graph.instagram.com/me").query(&[
            ("fields", "username,followers_count,media_count"),
            ("access_token", key),
        ]),
    )
    .await?;
    Ok(Count {
        second: Some(num(answer.get("media_count"))),
        note: answer.get("username").and_then(Value::as_str).map(str::to_owned),
        ..count(Service::Instagram, num(answer.get("followers_count")))
    })
}

/// X: followers, with the account's bearer token (needs an X API plan that allows lookups).
async fn x_profile(key: &str, username: &str) -> AppResult<Count> {
    let username = username.trim().trim_start_matches('@');
    if username.is_empty() {
        return Err(AppError::InvalidInput(
            "Add your X username in this card's popup.".into(),
        ));
    }
    let answer = json(
        Service::X,
        http()?
            .get(format!("https://api.x.com/2/users/by/username/{username}"))
            .bearer_auth(key)
            .query(&[("user.fields", "public_metrics")]),
    )
    .await?;
    Ok(Count {
        second: Some(num(answer.pointer("/data/public_metrics/following_count"))),
        note: Some(format!("@{username}")),
        ..count(
            Service::X,
            num(answer.pointer("/data/public_metrics/followers_count")),
        )
    })
}

/// DeepSeek: the account's balance.
async fn deepseek(key: &str) -> AppResult<Count> {
    let answer = json(
        Service::Deepseek,
        http()?
            .get("https://api.deepseek.com/user/balance")
            .bearer_auth(key),
    )
    .await?;
    let info = answer.pointer("/balance_infos/0");
    Ok(Count {
        note: info
            .and_then(|i| i.get("currency"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        second: info.map(|i| num(i.get("granted_balance"))),
        ..count(
            Service::Deepseek,
            info.map_or(0.0, |i| num(i.get("total_balance"))),
        )
    })
}

pub async fn counts(app: &AppHandle, service: Service) -> AppResult<Count> {
    let key = key_of(service)?;
    let cards = app.state::<SettingsStore>().get().cards;
    memo(&format!("count/{service:?}"), async move {
        match service {
            Service::Resend => resend(&key).await,
            Service::Linear => linear(&key).await,
            Service::Supabase => supabase(&key, &cards.supabase_project).await,
            Service::Youtube => youtube(&key, &cards.youtube_channel).await,
            Service::Instagram => instagram(&key).await,
            Service::X => x_profile(&key, &cards.x_username).await,
            Service::Deepseek => deepseek(&key).await,
            _ => Err(AppError::InvalidInput("not a counting service".into())),
        }
    })
    .await
}

// ---------- GitHub and Gmail (the accounts Dock already knows) ----------

/// Unread GitHub notifications, your open pull requests, and stars on your repositories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubOverview {
    pub notifications: u32,
    pub pull_requests: u32,
    pub stars: u32,
    pub login: String,
}

pub async fn github_overview() -> AppResult<GithubOverview> {
    let key = super::key(super::KeyAccount::Github)
        .ok_or_else(|| AppError::InvalidInput("Add a GitHub token in Settings → Accounts.".into()))?;
    memo("github/overview", async move {
        let client = http()?;
        let get = |path: &str| {
            client
                .get(format!("https://api.github.com{path}"))
                .bearer_auth(&key)
                .header("Accept", "application/vnd.github+json")
        };
        let gh = |r| async { json_named("GitHub", r).await };
        let user = gh(get("/user")).await?;
        let login = user
            .get("login")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let notifications = gh(get("/notifications?per_page=50")).await?;
        let prs = gh(get(&format!(
            "/search/issues?q=is:pr+is:open+author:{login}&per_page=1"
        )))
        .await?;
        let repos = gh(get("/user/repos?per_page=100&affiliation=owner")).await?;
        Ok(GithubOverview {
            notifications: notifications.as_array().map_or(0, Vec::len) as u32,
            pull_requests: num(prs.get("total_count")) as u32,
            stars: repos
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| num(r.get("stargazers_count")) as u32)
                .sum(),
            login,
        })
    })
    .await
}

async fn json_named(name: &str, request: reqwest::RequestBuilder) -> AppResult<Value> {
    let response = request.send().await.map_err(offline(name))?;
    let status = response.status();
    if !status.is_success() {
        return Err(problem(name, status.as_u16()));
    }
    response
        .json()
        .await
        .map_err(|_| AppError::InvalidInput(format!("{name} sent an answer Dock couldn't read.")))
}

/// Unread mail in the Gmail inbox, through the Google sign-in.
pub async fn gmail_unread(app: &AppHandle) -> AppResult<u32> {
    let token = super::google::token(app).await?;
    memo("gmail/unread", async move {
        let label = json_named(
            "Gmail",
            http()?
                .get("https://gmail.googleapis.com/gmail/v1/users/me/labels/INBOX")
                .bearer_auth(&token),
        )
        .await
        .map_err(|e| match e {
            AppError::InvalidInput(text) if text.contains("allowed") || text.contains("accept") => {
                AppError::InvalidInput(
                    "Sign in to Google again in Settings → Accounts to let Dock see Gmail's unread count."
                        .into(),
                )
            }
            other => other,
        })?;
        Ok(num(label.get("messagesUnread")) as u32)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn revenue_lands_in_its_day_and_old_sales_are_left_out() {
        let now = at("2026-11-15T12:00:00Z");
        let items = [
            (at("2026-11-15T01:00:00Z"), 10.0),
            (at("2026-11-15T23:00:00Z"), 5.0),
            (at("2026-11-01T10:00:00Z"), 2.5),
            (at("2026-09-01T10:00:00Z"), 99.0),
        ];
        let r = bucket(Service::Stripe, "usd", &items, now);
        assert_eq!(r.daily.len(), 30);
        assert_eq!(r.daily[29], 15.0);
        assert_eq!(r.total, 17.5);
        assert_eq!(r.currency, "USD");
    }

    #[test]
    fn traffic_compares_this_week_with_last() {
        let daily: Vec<f64> = (1..=14).map(f64::from).collect();
        let t = traffic_from_daily(Service::Dub, daily);
        assert_eq!(t.week, (8..=14).sum::<i32>() as f64);
        assert_eq!(t.previous, (1..=7).sum::<i32>() as f64);
    }

    #[test]
    fn reads_times_as_unix_seconds_and_dates() {
        assert_eq!(
            parse_time(&serde_json::json!(1_700_000_000)).unwrap().timestamp(),
            1_700_000_000
        );
        assert!(parse_time(&serde_json::json!("2026-11-07T05:31:56Z")).is_some());
        assert!(parse_time(&serde_json::json!("2026-11-07")).is_some());
    }

    #[test]
    fn every_service_has_its_own_secret_name() {
        let names: std::collections::HashSet<_> = Service::ALL.iter().map(|s| s.secret_name()).collect();
        assert_eq!(names.len(), Service::ALL.len());
        assert_eq!(Service::LemonSqueezy.secret_name(), "card-lemonSqueezy");
    }
}
