//! A focus timer: a stretch of work, then a short break, round again. The state is plain data
//! with the times it changes; the service sleeps until the next change rather than ticking.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use ts_rs::TS;

pub const FOCUS_MINUTES: i64 = 25;
pub const BREAK_MINUTES: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FocusPhase {
    Idle,
    Focus,
    Break,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusState {
    pub phase: FocusPhase,
    /// When the running phase ends (ISO time); `None` when idle or paused.
    #[ts(optional, type = "string")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ends_at: Option<DateTime<Utc>>,
    /// Seconds left while paused.
    #[ts(optional, type = "number")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused_seconds: Option<i64>,
    /// Length of the current phase, in seconds (for the progress ring).
    #[ts(type = "number")]
    pub phase_seconds: i64,
    /// Focus stretches finished since the dock started.
    pub completed: u32,
}

impl Default for FocusState {
    fn default() -> Self {
        Self {
            phase: FocusPhase::Idle,
            ends_at: None,
            paused_seconds: None,
            phase_seconds: FOCUS_MINUTES * 60,
            completed: 0,
        }
    }
}

impl FocusState {
    fn running(phase: FocusPhase, minutes: i64, now: DateTime<Utc>, completed: u32) -> Self {
        Self {
            phase,
            ends_at: Some(now + Duration::minutes(minutes)),
            paused_seconds: None,
            phase_seconds: minutes * 60,
            completed,
        }
    }

    /// Start focusing (from idle), or resume a paused phase.
    pub fn start(&self, now: DateTime<Utc>) -> Self {
        match (self.phase, self.paused_seconds) {
            (FocusPhase::Idle, _) => Self::running(FocusPhase::Focus, FOCUS_MINUTES, now, self.completed),
            (_, Some(left)) => Self {
                ends_at: Some(now + Duration::seconds(left)),
                paused_seconds: None,
                ..self.clone()
            },
            _ => self.clone(),
        }
    }

    pub fn pause(&self, now: DateTime<Utc>) -> Self {
        match self.ends_at {
            Some(ends) => Self {
                ends_at: None,
                paused_seconds: Some((ends - now).num_seconds().max(0)),
                ..self.clone()
            },
            None => self.clone(),
        }
    }

    /// Back to idle; the count of finished stretches stays.
    pub fn reset(&self) -> Self {
        Self {
            completed: self.completed,
            ..Self::default()
        }
    }

    /// Move on if the running phase has ended: focus → break (counting it), break → idle.
    pub fn advance(&self, now: DateTime<Utc>) -> Self {
        match (self.phase, self.ends_at) {
            (FocusPhase::Focus, Some(ends)) if ends <= now => {
                Self::running(FocusPhase::Break, BREAK_MINUTES, now, self.completed + 1)
            }
            (FocusPhase::Break, Some(ends)) if ends <= now => self.reset(),
            _ => self.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + secs, 0).unwrap()
    }

    #[test]
    fn a_full_round_counts_one_stretch() {
        let focus = FocusState::default().start(at(0));
        assert_eq!(focus.phase, FocusPhase::Focus);
        assert_eq!(focus.ends_at, Some(at(25 * 60)));
        // Not yet.
        assert_eq!(focus.advance(at(25 * 60 - 1)), focus);
        let rest = focus.advance(at(25 * 60));
        assert_eq!((rest.phase, rest.completed), (FocusPhase::Break, 1));
        assert_eq!(rest.ends_at, Some(at(30 * 60)));
        let done = rest.advance(at(30 * 60));
        assert_eq!((done.phase, done.completed), (FocusPhase::Idle, 1));
    }

    #[test]
    fn pausing_keeps_the_time_left() {
        let paused = FocusState::default().start(at(0)).pause(at(600));
        assert_eq!(paused.ends_at, None);
        assert_eq!(paused.paused_seconds, Some(15 * 60));
        // A paused timer never runs out.
        assert_eq!(paused.advance(at(99_999)), paused);
        let resumed = paused.start(at(1000));
        assert_eq!(resumed.ends_at, Some(at(1000 + 15 * 60)));
        assert_eq!(resumed.phase, FocusPhase::Focus);
    }

    #[test]
    fn reset_keeps_the_count() {
        let state = FocusState {
            completed: 3,
            ..FocusState::default().start(at(0))
        };
        let reset = state.reset();
        assert_eq!((reset.phase, reset.completed), (FocusPhase::Idle, 3));
    }
}
