//! Songs played today, built from the music widget's changes: a track is added when it starts
//! playing and isn't the one added last (pausing and resuming doesn't add it twice). The list
//! starts over each day.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::media::{MediaState, PlaybackStatus};

/// Most songs kept for one day.
const MAX_SONGS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlayedSong {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source_app: String,
    #[ts(type = "string")]
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SongsToday {
    /// The local day these songs were played ("2026-11-06").
    #[ts(type = "string | null")]
    pub day: Option<NaiveDate>,
    /// Oldest first.
    pub songs: Vec<PlayedSong>,
}

impl SongsToday {
    /// The list for `today`: emptied if it belongs to another day. Returns whether it changed.
    pub fn roll_over(&mut self, today: NaiveDate) -> bool {
        if self.day == Some(today) {
            return false;
        }
        let changed = self.day.is_some() || !self.songs.is_empty();
        self.day = Some(today);
        self.songs.clear();
        changed
    }

    /// Note what's playing now. Returns whether the list changed.
    pub fn observe(&mut self, music: Option<&MediaState>, today: NaiveDate, now: DateTime<Utc>) -> bool {
        let rolled = self.roll_over(today);
        let Some(music) = music else { return rolled };
        let title = music.title.trim();
        if music.status != PlaybackStatus::Playing || title.is_empty() {
            return rolled;
        }
        let artist = music.artist.trim();
        if self
            .songs
            .last()
            .is_some_and(|s| s.title == title && s.artist == artist)
        {
            return rolled;
        }
        self.songs.push(PlayedSong {
            title: title.to_string(),
            artist: artist.to_string(),
            album: music.album.trim().to_string(),
            source_app: music.source_app.clone(),
            at: now,
        });
        if self.songs.len() > MAX_SONGS {
            self.songs.remove(0);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaCapabilities;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + secs, 0).unwrap()
    }

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
    }

    fn playing(title: &str, status: PlaybackStatus) -> MediaState {
        MediaState {
            source_app: "Spotify".into(),
            source_icon: String::new(),
            title: title.into(),
            artist: "Artist".into(),
            album: "Album".into(),
            artwork: None,
            status,
            position_ms: None,
            duration_ms: None,
            position_updated_at: at(0),
            rate: 1.0,
            capabilities: MediaCapabilities::default(),
        }
    }

    #[test]
    fn a_track_is_added_once_while_it_plays() {
        let mut songs = SongsToday::default();
        assert!(songs.observe(Some(&playing("One", PlaybackStatus::Playing)), day(6), at(0)));
        // Pause, resume: still one.
        assert!(!songs.observe(Some(&playing("One", PlaybackStatus::Paused)), day(6), at(1)));
        assert!(!songs.observe(Some(&playing("One", PlaybackStatus::Playing)), day(6), at(2)));
        assert!(!songs.observe(None, day(6), at(3)));
        assert!(songs.observe(Some(&playing("Two", PlaybackStatus::Playing)), day(6), at(4)));
        // Back to the first: played again, so listed again.
        assert!(songs.observe(Some(&playing("One", PlaybackStatus::Playing)), day(6), at(5)));
        let titles: Vec<_> = songs.songs.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["One", "Two", "One"]);
    }

    #[test]
    fn paused_or_untitled_media_is_not_added() {
        let mut songs = SongsToday::default();
        songs.observe(Some(&playing("One", PlaybackStatus::Paused)), day(6), at(0));
        songs.observe(Some(&playing("  ", PlaybackStatus::Playing)), day(6), at(0));
        assert!(songs.songs.is_empty());
    }

    #[test]
    fn a_new_day_starts_over() {
        let mut songs = SongsToday::default();
        songs.observe(Some(&playing("One", PlaybackStatus::Playing)), day(6), at(0));
        assert!(songs.roll_over(day(7)));
        assert!(songs.songs.is_empty());
        assert_eq!(songs.day, Some(day(7)));
        assert!(!songs.roll_over(day(7)));
    }
}
