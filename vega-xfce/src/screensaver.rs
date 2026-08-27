use crate::i18n::gettext;
use gtk::{gio, gio::prelude::*};

const SCREENSAVER_SCHEMA: &str = "org.gnome.desktop.screensaver";
const SESSION_SCHEMA: &str = "org.gnome.desktop.session";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreensaverSettings {
    pub lock_enabled: bool,
    pub lock_delay_secs: u32,
    pub idle_delay_secs: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreensaverError(String);

impl std::fmt::Display for ScreensaverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ScreensaverError {}

/// Mesmo caso do papel de parede: bloqueio de tela é preferência da sessão
/// do usuário (dconf), não configuração de sistema — sem passar pelo vegad.
pub fn schema_available() -> bool {
    let Some(source) = gio::SettingsSchemaSource::default() else {
        return false;
    };
    source.lookup(SCREENSAVER_SCHEMA, true).is_some()
        && source.lookup(SESSION_SCHEMA, true).is_some()
}

pub fn current() -> Option<ScreensaverSettings> {
    if !schema_available() {
        return None;
    }
    let screensaver = gio::Settings::new(SCREENSAVER_SCHEMA);
    let session = gio::Settings::new(SESSION_SCHEMA);
    Some(ScreensaverSettings {
        lock_enabled: screensaver.boolean("lock-enabled"),
        lock_delay_secs: screensaver.uint("lock-delay"),
        idle_delay_secs: session.uint("idle-delay"),
    })
}

pub fn apply(settings: &ScreensaverSettings) -> Result<(), ScreensaverError> {
    if !schema_available() {
        return Err(ScreensaverError(gettext(
            "Os esquemas de bloqueio de tela não estão disponíveis neste sistema.",
        )));
    }
    let screensaver = gio::Settings::new(SCREENSAVER_SCHEMA);
    let session = gio::Settings::new(SESSION_SCHEMA);
    let _ = screensaver.set_boolean("lock-enabled", settings.lock_enabled);
    let _ = screensaver.set_uint("lock-delay", settings.lock_delay_secs);
    let _ = session.set_uint("idle-delay", settings.idle_delay_secs);
    Ok(())
}
