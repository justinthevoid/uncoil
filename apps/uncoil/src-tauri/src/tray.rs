//! The tray icon: switch the desk effect, open the window, quit. Switching edits config.json the same way
//! the window does (the engine hot-reloads it) and tells the window to reload its copy.

use std::collections::HashMap;
use std::sync::Mutex;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use uncoil_core::config::Config;
use uncoil_core::effect::Effect;

const TRAY_ID: &str = "main";
const OPEN: &str = "open";
const QUIT: &str = "quit";
const EFFECT_PREFIX: &str = "effect:";

/// The effects the menu offers, with the names the window uses (src/lib/effects.ts), plus Studio.
pub const EFFECTS: &[(&str, &str)] = &[
    ("wave", "Wave"),
    ("spectrum", "Spectrum"),
    ("breathing", "Breathing"),
    ("static", "Static"),
    ("starlight", "Starlight"),
    ("fire", "Fire"),
    ("wheel", "Wheel"),
    ("reactive", "Reactive"),
    ("ripple", "Ripple"),
    ("audio_meter", "Audio meter"),
    ("off", "Off"),
];

/// What the tray remembers this session: each effect's last settings, so flipping back keeps them.
#[derive(Default)]
pub struct TrayState {
    remembered: Mutex<HashMap<String, Effect>>,
    /// The effect kind the menu shows as chosen, and whether it offers Studio.
    shown: Mutex<Option<(String, bool)>>,
}

impl TrayState {
    /// Remember an effect the user is leaving.
    pub fn remember(&self, effect: &Effect) {
        if let (Some(kind), Ok(mut r)) = (kind_of(effect), self.remembered.lock()) {
            r.insert(kind, effect.clone());
        }
    }
}

/// `"wave"`, `"studio"`, …: the effect's serde tag.
pub fn kind_of(effect: &Effect) -> Option<String> {
    serde_json::to_value(effect).ok()?.get("kind")?.as_str().map(String::from)
}

/// An effect with its default settings. Static starts on the Amber gel, like the window's; Studio has no
/// default (it is only offered when there is one to go back to).
pub fn default_effect(kind: &str) -> Option<Effect> {
    let src = match kind {
        "studio" => return None,
        "static" => serde_json::json!({ "kind": "static", "color": [224, 163, 62] }),
        k => serde_json::json!({ "kind": k }),
    };
    serde_json::from_value(src).ok()
}

fn build_menu<R: Runtime>(app: &AppHandle<R>, current: &str, studio: bool) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    let mut list: Vec<(&str, &str)> = EFFECTS.to_vec();
    if studio {
        list.push(("studio", "Studio"));
    }
    for (kind, label) in list {
        let item =
            CheckMenuItem::with_id(app, format!("{EFFECT_PREFIX}{kind}"), label, true, kind == current, None::<&str>)?;
        menu.append(&item)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, OPEN, "Open uncoil", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, QUIT, "Quit", true, None::<&str>)?)?;
    Ok(menu)
}

/// Rebuild the menu if the chosen effect changed since it was built (the window or the file changed it).
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let effect = Config::load_reporting().0.effect;
    let current = kind_of(&effect).unwrap_or_default();
    let state = app.state::<TrayState>();
    let studio = current == "studio" || state.remembered.lock().is_ok_and(|r| r.contains_key("studio"));
    let want = Some((current.clone(), studio));
    let Ok(mut shown) = state.shown.lock() else { return };
    if *shown == want {
        return;
    }
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app, &current, studio)) {
        if tray.set_menu(Some(menu)).is_ok() {
            *shown = want;
        }
    }
}

/// Switch the desk effect from the menu. A config.json that can't be read is left alone rather than
/// replaced with defaults.
fn switch_effect<R: Runtime>(app: &AppHandle<R>, kind: &str) {
    let (mut config, problem) = Config::load_reporting();
    if problem.is_some() || kind_of(&config.effect).as_deref() == Some(kind) {
        refresh(app);
        return;
    }
    let state = app.state::<TrayState>();
    state.remember(&config.effect);
    let next = state.remembered.lock().ok().and_then(|r| r.get(kind).cloned()).or_else(|| default_effect(kind));
    let Some(next) = next else { return };
    config.effect = next;
    if config.save().is_ok() {
        let _ = app.emit("config-changed", ());
    }
    refresh(app);
}

/// Show the main window (sizing it first if it never opened) and bring it forward.
pub fn open_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        crate::show_window(&window);
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let effect = Config::load_reporting().0.effect;
    let current = kind_of(&effect).unwrap_or_default();
    let studio = current == "studio";
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("uncoil")
        .menu(&build_menu(app, &current, studio)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN => open_window(app),
            QUIT => app.exit(0),
            id => {
                if let Some(kind) = id.strip_prefix(EFFECT_PREFIX) {
                    switch_effect(app, kind);
                }
            }
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => {
                open_window(tray.app_handle())
            }
            TrayIconEvent::Enter { .. } => refresh(tray.app_handle()),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    if let Ok(mut shown) = app.state::<TrayState>().shown.lock() {
        *shown = Some((current, studio));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_effect_has_a_default_and_round_trips_its_kind() {
        for (kind, _) in EFFECTS {
            let e = default_effect(kind).unwrap_or_else(|| panic!("{kind} has no default"));
            assert_eq!(kind_of(&e).as_deref(), Some(*kind));
        }
        assert!(default_effect("studio").is_none());
    }

    /// The menu names match the window's effect cards (src/lib/effects.ts).
    #[test]
    fn menu_names_match_the_window() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/effects.ts");
        let ts = std::fs::read_to_string(path).unwrap();
        for (kind, label) in EFFECTS {
            assert!(
                ts.contains(&format!("kind: '{kind}', label: '{label}'")),
                "effects.ts has no {kind} named {label}"
            );
        }
        assert_eq!(ts.matches("label: '").count(), EFFECTS.len(), "effects.ts and the tray list different effects");
    }

    #[test]
    fn remembered_settings_come_back() {
        let state = TrayState::default();
        let custom: Effect = serde_json::from_str(r#"{"kind":"static","color":[1,2,3]}"#).unwrap();
        state.remember(&custom);
        assert_eq!(state.remembered.lock().unwrap().get("static"), Some(&custom));
    }
}
