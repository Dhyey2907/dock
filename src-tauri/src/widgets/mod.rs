//! The data behind the dock's widgets. Each source pushes changes as events; the page never
//! polls. Sources run only while a widget that shows them is on.

#[cfg(windows)]
pub mod audio;
pub mod focus;
pub mod network;
pub mod service;
pub mod stats;
pub mod weather;

use serde::Serialize;
use ts_rs::TS;

pub const WIDGET_WEATHER: &str = "widget-weather";
pub const WIDGET_STATS: &str = "widget-stats";
pub const WIDGET_SOUND: &str = "widget-sound";
pub const WIDGET_NETWORK: &str = "widget-network";
pub const WIDGET_FOCUS: &str = "widget-focus";
pub const WIDGET_MUSIC: &str = "widget-music";

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SoundState {
    /// 0.0 – 1.0.
    pub level: f32,
    pub muted: bool,
    /// Output device, e.g. "Speakers" or "Galaxy Buds2 Pro".
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

/// Everything the widgets show, in one read (for a page that just opened).
#[derive(Debug, Clone, PartialEq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WidgetSnapshot {
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather: Option<weather::Weather>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<stats::SystemStats>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<SoundState>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<network::NetworkState>,
    pub focus: focus::FocusState,
    /// What's playing; `None` when nothing is.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music: Option<crate::media::MediaState>,
}
