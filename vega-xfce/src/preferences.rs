use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub start_page: String,
    pub confirm_actions: bool,
    pub refresh_interval_minutes: u32,
    pub notify_updates: bool,
    pub notify_service_failures: bool,
    pub notify_backups: bool,
    pub redact_ai_data: bool,
    pub save_ai_history: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_page: "dashboard".into(),
            confirm_actions: true,
            refresh_interval_minutes: 5,
            notify_updates: true,
            notify_service_failures: true,
            notify_backups: true,
            redact_ai_data: true,
            save_ai_history: true,
        }
    }
}

static CONFIRM_ACTIONS: AtomicBool = AtomicBool::new(true);

fn path() -> PathBuf {
    gtk::glib::user_config_dir()
        .join("vega-gtk")
        .join("preferences.json")
}

pub fn load() -> Settings {
    let settings: Settings = fs::read_to_string(path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    CONFIRM_ACTIONS.store(settings.confirm_actions, Ordering::Relaxed);
    settings
}

pub fn save(settings: &Settings) {
    let path = path();
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return;
    }
    if let Ok(raw) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, raw);
    }
    CONFIRM_ACTIONS.store(settings.confirm_actions, Ordering::Relaxed);
}

pub fn confirmations_enabled() -> bool {
    CONFIRM_ACTIONS.load(Ordering::Relaxed)
}

pub fn refresh_interval_minutes() -> u32 {
    load().refresh_interval_minutes.clamp(1, 60)
}

pub fn notifications() -> (bool, bool, bool) {
    let settings = load();
    (
        settings.notify_updates,
        settings.notify_service_failures,
        settings.notify_backups,
    )
}

pub fn redact_ai_data() -> bool {
    load().redact_ai_data
}

pub fn save_ai_history() -> bool {
    load().save_ai_history
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe() {
        let settings = Settings::default();
        assert_eq!(settings.start_page, "dashboard");
        assert!(settings.confirm_actions);
    }
}
