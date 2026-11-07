//! Widgets outside the dock: a popup detached into its own small window that stays on top, or
//! one kept on the desktop (behind other windows). Each lives in a window labelled
//! `float-<kind>`, and where it sits is saved so it comes back after a restart. The dashboard
//! is one window showing every widget on the dock at once.

use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use super::panel::{self, PanelContent, PANEL_LABEL};
use crate::{
    error::AppResult,
    settings::{FloatingWidget, SettingsStore, WidgetKind},
};

const DASHBOARD_LABEL: &str = "dashboard";
const PREFIX: &str = "float-";
/// Room above the widget's own popup for the floating window's title bar, logical pixels.
const HEADER: f64 = 30.0;

/// The kind's name as the page and window labels spell it ("clock", "music").
fn kind_name(kind: WidgetKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn label(kind: WidgetKind) -> String {
    format!("{PREFIX}{}", kind_name(kind))
}

/// Show `kind` in its own window at `position` (physical pixels), on top or on the desktop.
fn open(app: &AppHandle, kind: WidgetKind, position: Option<(i32, i32)>, on_top: bool) -> AppResult<()> {
    let label = label(kind);
    let window = match app.get_webview_window(&label) {
        Some(window) => window,
        None => {
            let (width, height) = panel::panel_size(kind);
            let url = format!("index.html#float/{}", kind_name(kind));
            let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
                .title("Dock widget")
                .inner_size(width, height + HEADER)
                .decorations(false)
                .transparent(true)
                .shadow(true)
                .skip_taskbar(true)
                .resizable(false)
                .effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![tauri::window::Effect::Acrylic],
                    ..Default::default()
                });
            if position.is_none() {
                builder = builder.center();
            }
            builder.build()?
        }
    };
    if let Some((x, y)) = position {
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    window.set_always_on_top(on_top)?;
    window.set_always_on_bottom(!on_top)?;
    window.show()?;
    #[cfg(windows)]
    super::make_tool_window(&window, false)?;
    Ok(())
}

/// Remember (or forget, with `None`) where `kind` floats.
fn remember(app: &AppHandle, kind: WidgetKind, entry: Option<FloatingWidget>) -> AppResult<()> {
    let store = app.state::<SettingsStore>();
    let mut settings = store.get();
    settings.floating.retain(|f| f.kind != kind);
    settings.floating.extend(entry);
    store.set(settings.clone())?;
    tauri::Emitter::emit(app, crate::commands::SETTINGS_CHANGED, &settings)?;
    Ok(())
}

/// Turn the open widget popup into a window that stays on top, where the popup was.
pub fn detach_popup(app: &AppHandle) -> AppResult<()> {
    let Some(PanelContent::Widget { kind }) = panel::current(app) else {
        return Ok(());
    };
    let position = app
        .get_webview_window(PANEL_LABEL)
        .and_then(|w| w.outer_position().ok())
        .map(|p| (p.x, p.y));
    panel::close(app)?;
    open(app, kind, position, true)?;
    let (x, y) = position.unwrap_or_default();
    remember(
        app,
        kind,
        Some(FloatingWidget {
            kind,
            x,
            y,
            on_top: true,
        }),
    )
}

/// Put `kind` on the desktop (behind other windows), or keep it on top.
pub fn show(app: &AppHandle, kind: WidgetKind, on_top: bool) -> AppResult<()> {
    let saved = app
        .state::<SettingsStore>()
        .get()
        .floating
        .into_iter()
        .find(|f| f.kind == kind);
    let position = saved.map(|f| (f.x, f.y));
    open(app, kind, position, on_top)?;
    let (x, y) = match app
        .get_webview_window(&label(kind))
        .and_then(|w| w.outer_position().ok())
    {
        Some(p) => (p.x, p.y),
        None => position.unwrap_or_default(),
    };
    remember(app, kind, Some(FloatingWidget { kind, x, y, on_top }))
}

/// The floating `kind` was dragged to `x`, `y` (physical pixels).
pub fn moved(app: &AppHandle, kind: WidgetKind, x: i32, y: i32) -> AppResult<()> {
    let store = app.state::<SettingsStore>();
    let Some(current) = store.get().floating.into_iter().find(|f| f.kind == kind) else {
        return Ok(());
    };
    if (current.x, current.y) == (x, y) {
        return Ok(());
    }
    remember(app, kind, Some(FloatingWidget { x, y, ..current }))
}

/// Close the floating `kind` for good.
pub fn close(app: &AppHandle, kind: WidgetKind) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(&label(kind)) {
        window.close()?;
    }
    remember(app, kind, None)
}

/// Bring back the floating widgets saved last time.
pub fn restore(app: &AppHandle) {
    for f in app.state::<SettingsStore>().get().floating {
        if let Err(error) = open(app, f.kind, Some((f.x, f.y)), f.on_top) {
            tracing::warn!(%error, kind = ?f.kind, "couldn't restore a floating widget");
        }
    }
}

/// Open (or focus) the dashboard: every widget on the dock, all at once.
pub fn open_dashboard(app: &AppHandle) -> AppResult<()> {
    panel::close(app)?;
    if let Some(window) = app.get_webview_window(DASHBOARD_LABEL) {
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(
        app,
        DASHBOARD_LABEL,
        WebviewUrl::App("index.html#dashboard".into()),
    )
    .title("Dock dashboard")
    .inner_size(1080.0, 740.0)
    .min_inner_size(520.0, 420.0)
    .transparent(true)
    .effects(super::mica())
    .center()
    .build()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_use_the_page_spelling() {
        assert_eq!(label(WidgetKind::Clock), "float-clock");
        assert_eq!(kind_name(WidgetKind::Calendar), "calendar");
    }
}
