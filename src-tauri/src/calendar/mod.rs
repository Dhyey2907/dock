//! Calendars from secret iCal links (Google, Outlook, iCloud). This file is the pure part:
//! reading iCalendar text and expanding events — repeats, skipped dates, moved occurrences and
//! time zones — into a list for a stretch of days. `service.rs` fetches and stores the links.

pub mod service;

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;
use ts_rs::TS;

/// An event occurrence, ready to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CalendarEvent {
    /// Unique per occurrence.
    pub id: String,
    pub calendar_id: String,
    pub title: String,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Start and end. All-day events run from local midnight to midnight.
    #[ts(type = "string")]
    pub start: DateTime<Utc>,
    #[ts(type = "string")]
    pub end: DateTime<Utc>,
    pub all_day: bool,
}

/// One content line: `NAME;PARAM=value:VALUE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub value: String,
}

impl Property {
    fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Undo line folding (a line starting with a space or tab continues the previous one).
pub fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(rest) = line.strip_prefix([' ', '\t']) {
            if let Some(last) = lines.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        if !line.is_empty() {
            lines.push(line.to_string());
        }
    }
    lines
}

/// Split a content line into name, parameters and value (quoted parameter values may hold `:`).
pub fn parse_property(line: &str) -> Option<Property> {
    let mut in_quotes = false;
    let colon = line.char_indices().find_map(|(i, c)| {
        if c == '"' {
            in_quotes = !in_quotes;
        }
        (c == ':' && !in_quotes).then_some(i)
    })?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim_matches('"').to_string()))
        .collect();
    Some(Property {
        name,
        params,
        value: value.to_string(),
    })
}

/// Text values escape commas, semicolons, newlines and backslashes.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push(' '),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// A VEVENT as written, before expansion.
#[derive(Debug, Clone, Default)]
pub struct RawEvent {
    pub uid: String,
    pub summary: String,
    pub location: Option<String>,
    pub start: Option<Property>,
    pub end: Option<Property>,
    pub duration: Option<String>,
    pub rrule: Option<String>,
    pub exdates: Vec<Property>,
    pub rdates: Vec<Property>,
    pub recurrence_id: Option<Property>,
    pub cancelled: bool,
}

/// The events in an iCalendar document.
pub fn parse_events(text: &str) -> Vec<RawEvent> {
    let mut events = Vec::new();
    let mut current: Option<RawEvent> = None;
    // Alarms and other parts nested in an event have their own DTSTART etc.; skip them.
    let mut nested = 0usize;
    for line in unfold(text) {
        let Some(p) = parse_property(&line) else { continue };
        match (p.name.as_str(), p.value.to_ascii_uppercase().as_str()) {
            ("BEGIN", "VEVENT") => current = Some(RawEvent::default()),
            ("END", "VEVENT") => {
                if let Some(event) = current.take() {
                    events.push(event);
                }
                nested = 0;
            }
            ("BEGIN", _) if current.is_some() => nested += 1,
            ("END", _) if current.is_some() => nested = nested.saturating_sub(1),
            _ => {
                let Some(event) = current.as_mut().filter(|_| nested == 0) else {
                    continue;
                };
                match p.name.as_str() {
                    "UID" => event.uid = p.value.clone(),
                    "SUMMARY" => event.summary = unescape(&p.value),
                    "LOCATION" => event.location = Some(unescape(&p.value)).filter(|l| !l.is_empty()),
                    "DTSTART" => event.start = Some(p),
                    "DTEND" => event.end = Some(p),
                    "DURATION" => event.duration = Some(p.value),
                    "RRULE" => event.rrule = Some(p.value),
                    "EXDATE" => event.exdates.push(p),
                    "RDATE" => event.rdates.push(p),
                    "RECURRENCE-ID" => event.recurrence_id = Some(p),
                    "STATUS" => event.cancelled = p.value.eq_ignore_ascii_case("CANCELLED"),
                    _ => {}
                }
            }
        }
    }
    events
}

/// Windows time zone names (Outlook writes these) to IANA names.
pub fn iana_zone(tzid: &str) -> Option<chrono_tz::Tz> {
    let tzid = tzid.trim().trim_start_matches('/');
    if let Ok(tz) = tzid.parse::<chrono_tz::Tz>() {
        return Some(tz);
    }
    let iana = match tzid {
        "India Standard Time" => "Asia/Kolkata",
        "UTC" | "Coordinated Universal Time" | "GMT Standard Time" => "Europe/London",
        "Pacific Standard Time" => "America/Los_Angeles",
        "Mountain Standard Time" => "America/Denver",
        "Central Standard Time" => "America/Chicago",
        "Eastern Standard Time" => "America/New_York",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Romance Standard Time" => "Europe/Paris",
        "Central Europe Standard Time" => "Europe/Budapest",
        "E. Europe Standard Time" => "Europe/Bucharest",
        "Russian Standard Time" => "Europe/Moscow",
        "Arabian Standard Time" => "Asia/Dubai",
        "Pakistan Standard Time" => "Asia/Karachi",
        "Bangladesh Standard Time" => "Asia/Dhaka",
        "SE Asia Standard Time" => "Asia/Bangkok",
        "China Standard Time" => "Asia/Shanghai",
        "Singapore Standard Time" => "Asia/Singapore",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "New Zealand Standard Time" => "Pacific/Auckland",
        "E. South America Standard Time" => "America/Sao_Paulo",
        _ => return None,
    };
    iana.parse().ok()
}

/// A DTSTART-style value as a moment, and whether it is a whole day.
pub fn parse_moment(p: &Property) -> Option<(DateTime<Utc>, bool)> {
    let value = p.value.trim();
    let date_only = p.param("VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE")) || value.len() == 8;
    if date_only {
        let date = NaiveDate::parse_from_str(value.get(..8)?, "%Y%m%d").ok()?;
        let midnight = Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
            .earliest()?;
        return Some((midnight.with_timezone(&Utc), true));
    }
    if let Some(utc) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some((Utc.from_utc_datetime(&naive), false));
    }
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    let at = match p.param("TZID").and_then(iana_zone) {
        Some(tz) => tz.from_local_datetime(&naive).earliest()?.with_timezone(&Utc),
        // A floating time, or a zone we don't know: the PC's own time.
        None => Local.from_local_datetime(&naive).earliest()?.with_timezone(&Utc),
    };
    Some((at, false))
}

/// An ISO 8601 duration such as `PT1H30M` or `P1D`.
pub fn parse_duration(value: &str) -> Option<Duration> {
    let (negative, value) = match value.trim().strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, value.trim().trim_start_matches('+')),
    };
    let body = value.strip_prefix('P')?;
    let mut total = Duration::zero();
    let mut number = String::new();
    let mut in_time = false;
    for c in body.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => number.push(c),
            unit => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                total += match (unit, in_time) {
                    ('W', _) => Duration::weeks(n),
                    ('D', _) => Duration::days(n),
                    ('H', true) => Duration::hours(n),
                    ('M', true) => Duration::minutes(n),
                    ('S', true) => Duration::seconds(n),
                    _ => return None,
                };
            }
        }
    }
    Some(if negative { -total } else { total })
}

/// The rule text `rrule` expands: DTSTART in the event's zone (so repeats keep their local time
/// across daylight saving), the RRULE, and skipped dates as UTC moments.
fn rule_text(
    start: &Property,
    start_at: DateTime<Utc>,
    all_day: bool,
    rule: &str,
    exdates: &[DateTime<Utc>],
) -> String {
    let dtstart = match start.param("TZID").and_then(iana_zone) {
        Some(tz) if !all_day => {
            format!(
                "DTSTART;TZID={}:{}",
                tz.name(),
                start_at.with_timezone(&tz).format("%Y%m%dT%H%M%S")
            )
        }
        _ => format!("DTSTART:{}", start_at.format("%Y%m%dT%H%M%SZ")),
    };
    let mut text = format!("{dtstart}\nRRULE:{rule}");
    for ex in exdates {
        text.push_str(&format!("\nEXDATE:{}", ex.format("%Y%m%dT%H%M%SZ")));
    }
    text
}

/// Most occurrences taken from one repeating event (guards against runaway rules).
const MAX_OCCURRENCES: u16 = 500;

/// Every occurrence of `events` that overlaps `[from, to)`, sorted by start.
pub fn expand(
    events: &[RawEvent],
    calendar_id: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<CalendarEvent> {
    // Moved or edited single occurrences, by (uid, original start).
    let mut overridden: HashSet<(String, DateTime<Utc>)> = HashSet::new();
    for event in events {
        if let Some((original, _)) = event.recurrence_id.as_ref().and_then(parse_moment) {
            overridden.insert((event.uid.clone(), original));
        }
    }

    let mut out = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for event in events.iter().filter(|e| !e.cancelled) {
        let Some(start_prop) = &event.start else { continue };
        let Some((start, all_day)) = parse_moment(start_prop) else {
            continue;
        };
        let length = match (&event.end, &event.duration) {
            (Some(end), _) => parse_moment(end).map(|(end, _)| end - start),
            (None, Some(d)) => parse_duration(d),
            (None, None) => None,
        }
        .filter(|d| *d >= Duration::zero())
        .unwrap_or(if all_day {
            Duration::days(1)
        } else {
            Duration::zero()
        });

        let mut starts = vec![start];
        if event.recurrence_id.is_none() {
            if let Some(rule) = &event.rrule {
                let exdates: Vec<DateTime<Utc>> = event
                    .exdates
                    .iter()
                    .flat_map(|p| {
                        // EXDATE may list several values separated by commas.
                        p.value.split(',').filter_map(|v| {
                            parse_moment(&Property {
                                value: v.to_string(),
                                ..p.clone()
                            })
                            .map(|(at, _)| at)
                        })
                    })
                    .collect();
                let text = rule_text(start_prop, start, all_day, rule, &exdates);
                match text.parse::<rrule::RRuleSet>() {
                    Ok(set) => {
                        let window_start = (from - length).with_timezone(&rrule::Tz::UTC);
                        let result = set
                            .after(window_start)
                            .before(to.with_timezone(&rrule::Tz::UTC))
                            .all(MAX_OCCURRENCES);
                        starts = result.dates.iter().map(|d| d.with_timezone(&Utc)).collect();
                    }
                    Err(error) => tracing::debug!(%error, "skipping a repeat rule we can't read"),
                }
            }
            for p in &event.rdates {
                starts.extend(p.value.split(',').filter_map(|v| {
                    parse_moment(&Property {
                        value: v.to_string(),
                        ..p.clone()
                    })
                    .map(|(at, _)| at)
                }));
            }
        }

        for at in starts {
            let end = at + length;
            // A repeat whose occurrence was moved is shown where it moved to, not here.
            if event.recurrence_id.is_none()
                && event.rrule.is_some()
                && overridden.contains(&(event.uid.clone(), at))
            {
                continue;
            }
            // Zero-length events still show at their moment.
            if end < from || at >= to || (end == from && length > Duration::zero()) {
                continue;
            }
            let key = format!("{calendar_id}/{}/{}", event.uid, at.timestamp());
            let n = seen.entry(key.clone()).or_insert(0);
            *n += 1;
            out.push(CalendarEvent {
                id: if *n == 1 { key } else { format!("{key}/{n}") },
                calendar_id: calendar_id.to_string(),
                title: if event.summary.is_empty() {
                    "Busy".into()
                } else {
                    event.summary.clone()
                },
                location: event.location.clone(),
                start: at,
                end,
                all_day,
            });
        }
    }
    out.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.title.cmp(&b.title)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    const CAL: &str = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
BEGIN:VEVENT\r\n\
UID:standup@x\r\n\
SUMMARY:Daily standup\r\n\
DTSTART;TZID=India Standard Time:20261102T100000\r\n\
DTEND;TZID=India Standard Time:20261102T101500\r\n\
RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\r\n\
EXDATE;TZID=India Standard Time:20261104T100000\r\n\
BEGIN:VALARM\r\n\
TRIGGER:-PT10M\r\n\
DTSTART:19700101T000000Z\r\n\
END:VALARM\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:standup@x\r\n\
RECURRENCE-ID;TZID=India Standard Time:20261105T100000\r\n\
SUMMARY:Daily standup (moved)\r\n\
DTSTART;TZID=India Standard Time:20261105T113000\r\n\
DTEND;TZID=India Standard Time:20261105T114500\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:review@x\r\n\
SUMMARY:Design review\\, round 2\r\n\
LOCATION:Room 4\r\n\
DTSTART:20261105T090000Z\r\n\
DURATION:PT1H30M\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:holiday@x\r\n\
SUMMARY:Diwali\r\n\
DTSTART;VALUE=DATE:20261108\r\n\
DTEND;VALUE=DATE:20261109\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:gone@x\r\n\
SUMMARY:Cancelled thing\r\n\
STATUS:CANCELLED\r\n\
DTSTART:20261105T120000Z\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

    #[test]
    fn unfolds_and_splits_lines() {
        let lines = unfold("SUMMARY:Long\r\n  title\r\nUID:1\r\n");
        assert_eq!(lines, vec!["SUMMARY:Long title", "UID:1"]);
        let p = parse_property("DTSTART;TZID=\"Europe/Paris\";X=a:b:20261105T100000").unwrap();
        assert_eq!(p.name, "DTSTART");
        assert_eq!(p.param("tzid"), Some("Europe/Paris"));
        assert_eq!(p.value, "b:20261105T100000");
    }

    #[test]
    fn reads_events_but_not_their_alarms() {
        let events = parse_events(CAL);
        assert_eq!(events.len(), 5);
        // The alarm's DTSTART didn't replace the event's.
        assert_eq!(events[0].start.as_ref().unwrap().value, "20261102T100000");
        assert_eq!(events[2].summary, "Design review, round 2");
        assert!(events[4].cancelled);
    }

    #[test]
    fn expands_repeats_with_skips_moves_and_zones() {
        let events = parse_events(CAL);
        let list = expand(
            &events,
            "work",
            utc("2026-11-02T00:00:00Z"),
            utc("2026-11-07T00:00:00Z"),
        );
        let titles: Vec<(&str, String)> = list
            .iter()
            .filter(|e| !e.all_day)
            .map(|e| (e.title.as_str(), e.start.to_rfc3339()))
            .collect();
        assert_eq!(
            titles,
            vec![
                // 10:00 India time is 04:30 UTC.
                ("Daily standup", "2026-11-02T04:30:00+00:00".into()),
                ("Daily standup", "2026-11-03T04:30:00+00:00".into()),
                // Wednesday skipped (EXDATE); Thursday's occurrence moved to 11:30.
                ("Daily standup (moved)", "2026-11-05T06:00:00+00:00".into()),
                ("Design review, round 2", "2026-11-05T09:00:00+00:00".into()),
                ("Daily standup", "2026-11-06T04:30:00+00:00".into()),
            ]
        );
        let review = list.iter().find(|e| e.title.starts_with("Design")).unwrap();
        assert_eq!(review.end - review.start, Duration::minutes(90));
        assert_eq!(review.location.as_deref(), Some("Room 4"));
        assert!(list.iter().all(|e| e.title != "Cancelled thing"));
    }

    #[test]
    fn all_day_events_cover_their_local_day() {
        let events = parse_events(CAL);
        let list = expand(
            &events,
            "work",
            utc("2026-11-07T00:00:00Z"),
            utc("2026-11-10T00:00:00Z"),
        );
        let diwali = list.iter().find(|e| e.title == "Diwali").unwrap();
        assert!(diwali.all_day);
        assert_eq!(diwali.end - diwali.start, Duration::days(1));
    }

    #[test]
    fn ids_are_unique_per_occurrence() {
        let events = parse_events(CAL);
        let list = expand(
            &events,
            "work",
            utc("2026-11-01T00:00:00Z"),
            utc("2026-11-30T00:00:00Z"),
        );
        let ids: HashSet<&String> = list.iter().map(|e| &e.id).collect();
        assert_eq!(ids.len(), list.len());
    }

    #[test]
    fn parses_durations_and_zone_names() {
        assert_eq!(parse_duration("PT1H30M"), Some(Duration::minutes(90)));
        assert_eq!(parse_duration("P1DT2H"), Some(Duration::hours(26)));
        assert_eq!(parse_duration("-PT10M"), Some(Duration::minutes(-10)));
        assert_eq!(parse_duration("1H"), None);
        assert_eq!(iana_zone("India Standard Time").unwrap().name(), "Asia/Kolkata");
        assert_eq!(iana_zone("America/New_York").unwrap().name(), "America/New_York");
        assert!(iana_zone("Mars Standard Time").is_none());
    }
}
