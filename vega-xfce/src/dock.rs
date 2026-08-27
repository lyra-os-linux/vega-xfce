use std::path::PathBuf;

use crate::i18n::gettext;
use gtk::{gio, gio::prelude::*, glib};

const EXTENSION_UUID: &str = "sheliak@lyraos.com.br";
const SCHEMA_ID: &str = "org.gnome.shell.extensions.sheliak";
const SCHEMA_PATH: &str = "/org/gnome/shell/extensions/sheliak/";
const SHELL_SCHEMA_ID: &str = "org.gnome.shell";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSettings {
    pub position: String,
    pub hide_mode: String,
    pub hide_delay_ms: u32,
    pub icon_size: u32,
    pub edge_margin: u32,
    pub animation: bool,
    pub minimize_animation: String,
    pub extend_to_edges: bool,
    pub content_alignment: String,
    pub show_running: bool,
    pub running_apps_position: String,
    pub show_trash: bool,
    pub show_apps_button: bool,
    pub fullscreen_hide: bool,
}

/// Menus da barra superior (Aplicativos/Locais) e seu conteúdo — mesma
/// extensão Sheliak do dock, mas editados na aba "Menu" da Personalização.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSettings {
    pub panel_height: u32,
    pub floating_panel: bool,
    pub panel_margin: u32,
    pub show_clock: bool,
    pub show_panel_indicators: bool,
    pub show_applications_menu: bool,
    pub show_places_menu: bool,
    pub show_network_menu: bool,
    pub show_system_menu: bool,
    pub show_system_about: bool,
    pub show_search_menu: bool,
    pub hide_workspace_button: bool,
    pub panel_menu_position: String,
    pub show_application_icons: bool,
    pub sort_applications_menu: bool,
    pub open_application_submenus_sideways: bool,
    pub show_place_bookmarks: bool,
    pub show_place_volumes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockError(String);

impl std::fmt::Display for DockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for DockError {}

/// Ao contrário de wallpaper/screensaver, o schema do Sheliak não fica no
/// diretório global do glib-2.0: como toda extensão GNOME Shell, ele é
/// empacotado dentro do próprio diretório da extensão. Por isso precisamos
/// achar essa pasta e carregar o schema explicitamente dali, em vez de usar
/// `SettingsSchemaSource::default()`.
fn extension_dir() -> Option<PathBuf> {
    let mut candidates = vec![
        glib::user_data_dir()
            .join("gnome-shell/extensions")
            .join(EXTENSION_UUID),
    ];
    for base in ["/usr/share", "/usr/local/share"] {
        candidates.push(
            PathBuf::from(base)
                .join("gnome-shell/extensions")
                .join(EXTENSION_UUID),
        );
    }
    candidates
        .into_iter()
        .find(|dir| dir.join("metadata.json").is_file())
}

pub fn is_installed() -> bool {
    extension_dir().is_some()
}

fn shell_settings() -> Option<gio::Settings> {
    gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup(SHELL_SCHEMA_ID, true))
        .map(|_| gio::Settings::new(SHELL_SCHEMA_ID))
}

pub fn is_enabled() -> bool {
    shell_settings().is_some_and(|settings| {
        settings
            .strv("enabled-extensions")
            .iter()
            .any(|uuid| uuid.as_str() == EXTENSION_UUID)
    })
}

pub fn set_enabled(enabled: bool) -> Result<(), DockError> {
    if enabled && !is_installed() {
        return Err(DockError(gettext(
            "A extensão Sheliak não está instalada ou não pôde ser encontrada.",
        )));
    }
    let settings = shell_settings().ok_or_else(|| {
        DockError(gettext(
            "As configurações de extensões do GNOME não estão disponíveis.",
        ))
    })?;
    let mut extensions = settings
        .strv("enabled-extensions")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    extensions.retain(|uuid| uuid != EXTENSION_UUID);
    if enabled {
        extensions.push(EXTENSION_UUID.to_string());
    }
    let extension_refs = extensions.iter().map(String::as_str).collect::<Vec<_>>();
    settings
        .set_strv("enabled-extensions", extension_refs)
        .map_err(|_| {
            DockError(gettext(
                "Não foi possível alterar o perfil da área de trabalho.",
            ))
        })
}

fn open_settings() -> Option<gio::Settings> {
    let dir = extension_dir()?;
    let source = gio::SettingsSchemaSource::from_directory(
        dir.join("schemas"),
        gio::SettingsSchemaSource::default().as_ref(),
        false,
    )
    .ok()?;
    let schema = source.lookup(SCHEMA_ID, false)?;
    Some(gio::Settings::new_full(
        &schema,
        None::<&gio::SettingsBackend>,
        Some(SCHEMA_PATH),
    ))
}

fn has_key(settings: &gio::Settings, key: &str) -> bool {
    settings
        .settings_schema()
        .is_some_and(|schema| schema.has_key(key))
}

fn string_or(settings: &gio::Settings, key: &str, fallback: &str) -> String {
    if has_key(settings, key) {
        settings.string(key).to_string()
    } else {
        fallback.to_string()
    }
}

fn boolean_or(settings: &gio::Settings, key: &str, fallback: bool) -> bool {
    if has_key(settings, key) {
        settings.boolean(key)
    } else {
        fallback
    }
}

fn uint_or(settings: &gio::Settings, key: &str, fallback: u32) -> u32 {
    if has_key(settings, key) {
        settings.uint(key)
    } else {
        fallback
    }
}

fn set_string_if_present(settings: &gio::Settings, key: &str, value: &str) {
    if has_key(settings, key) {
        let _ = settings.set_string(key, value);
    }
}

fn set_boolean_if_present(settings: &gio::Settings, key: &str, value: bool) {
    if has_key(settings, key) {
        let _ = settings.set_boolean(key, value);
    }
}

fn set_uint_if_present(settings: &gio::Settings, key: &str, value: u32) {
    if has_key(settings, key) {
        let _ = settings.set_uint(key, value);
    }
}

pub fn current() -> Option<DockSettings> {
    let settings = open_settings()?;
    Some(DockSettings {
        position: settings.string("position").to_string(),
        hide_mode: settings.string("hide-mode").to_string(),
        hide_delay_ms: settings.uint("hide-delay"),
        icon_size: settings.uint("icon-size"),
        edge_margin: settings.uint("edge-margin"),
        animation: settings.boolean("animation"),
        minimize_animation: string_or(&settings, "minimize-animation", "zoom"),
        extend_to_edges: settings.boolean("extend-to-edges"),
        content_alignment: settings.string("content-alignment").to_string(),
        show_running: settings.boolean("show-running"),
        running_apps_position: settings.string("running-apps-position").to_string(),
        show_trash: settings.boolean("show-trash"),
        show_apps_button: settings.boolean("show-apps-button"),
        fullscreen_hide: settings.boolean("fullscreen-hide"),
    })
}

pub fn current_menu() -> Option<MenuSettings> {
    let settings = open_settings()?;
    Some(MenuSettings {
        panel_height: uint_or(&settings, "panel-height", 32),
        floating_panel: boolean_or(&settings, "floating-panel", true),
        panel_margin: uint_or(&settings, "panel-margin", 8),
        show_clock: boolean_or(&settings, "show-clock", true),
        show_panel_indicators: boolean_or(&settings, "show-panel-indicators", true),
        show_applications_menu: boolean_or(&settings, "show-applications-menu", true),
        show_places_menu: boolean_or(&settings, "show-places-menu", true),
        show_network_menu: boolean_or(&settings, "show-network-menu", true),
        show_system_menu: boolean_or(&settings, "show-system-menu", true),
        show_system_about: boolean_or(&settings, "show-system-about", true),
        show_search_menu: boolean_or(&settings, "show-search-menu", true),
        hide_workspace_button: boolean_or(&settings, "hide-workspace-button", true),
        panel_menu_position: string_or(&settings, "panel-menu-position", "left"),
        show_application_icons: boolean_or(&settings, "show-application-icons", true),
        sort_applications_menu: boolean_or(&settings, "sort-applications-menu", true),
        open_application_submenus_sideways: boolean_or(
            &settings,
            "open-application-submenus-sideways",
            true,
        ),
        show_place_bookmarks: boolean_or(&settings, "show-place-bookmarks", true),
        show_place_volumes: boolean_or(&settings, "show-place-volumes", true),
    })
}

pub fn apply(settings: &DockSettings) -> Result<(), DockError> {
    let gsettings = open_settings().ok_or_else(|| {
        DockError(gettext(
            "A extensão Sheliak não está instalada ou não pôde ser encontrada.",
        ))
    })?;
    let _ = gsettings.set_string("position", &settings.position);
    let _ = gsettings.set_string("hide-mode", &settings.hide_mode);
    let _ = gsettings.set_uint("hide-delay", settings.hide_delay_ms);
    let _ = gsettings.set_uint("icon-size", settings.icon_size);
    let _ = gsettings.set_uint("edge-margin", settings.edge_margin);
    let _ = gsettings.set_boolean("animation", settings.animation);
    set_string_if_present(
        &gsettings,
        "minimize-animation",
        &settings.minimize_animation,
    );
    let _ = gsettings.set_boolean("extend-to-edges", settings.extend_to_edges);
    let _ = gsettings.set_string("content-alignment", &settings.content_alignment);
    let _ = gsettings.set_boolean("show-running", settings.show_running);
    let _ = gsettings.set_string("running-apps-position", &settings.running_apps_position);
    let _ = gsettings.set_boolean("show-trash", settings.show_trash);
    let _ = gsettings.set_boolean("show-apps-button", settings.show_apps_button);
    let _ = gsettings.set_boolean("fullscreen-hide", settings.fullscreen_hide);
    Ok(())
}

pub fn apply_menu(settings: &MenuSettings) -> Result<(), DockError> {
    let gsettings = open_settings().ok_or_else(|| {
        DockError(gettext(
            "A extensão Sheliak não está instalada ou não pôde ser encontrada.",
        ))
    })?;
    set_uint_if_present(&gsettings, "panel-height", settings.panel_height);
    set_boolean_if_present(&gsettings, "floating-panel", settings.floating_panel);
    set_uint_if_present(&gsettings, "panel-margin", settings.panel_margin);
    set_boolean_if_present(&gsettings, "show-clock", settings.show_clock);
    set_boolean_if_present(
        &gsettings,
        "show-panel-indicators",
        settings.show_panel_indicators,
    );
    set_boolean_if_present(
        &gsettings,
        "show-applications-menu",
        settings.show_applications_menu,
    );
    set_boolean_if_present(&gsettings, "show-places-menu", settings.show_places_menu);
    set_boolean_if_present(&gsettings, "show-network-menu", settings.show_network_menu);
    set_boolean_if_present(&gsettings, "show-system-menu", settings.show_system_menu);
    set_boolean_if_present(&gsettings, "show-system-about", settings.show_system_about);
    set_boolean_if_present(&gsettings, "show-search-menu", settings.show_search_menu);
    set_boolean_if_present(
        &gsettings,
        "hide-workspace-button",
        settings.hide_workspace_button,
    );
    set_string_if_present(
        &gsettings,
        "panel-menu-position",
        &settings.panel_menu_position,
    );
    set_boolean_if_present(
        &gsettings,
        "show-application-icons",
        settings.show_application_icons,
    );
    set_boolean_if_present(
        &gsettings,
        "sort-applications-menu",
        settings.sort_applications_menu,
    );
    set_boolean_if_present(
        &gsettings,
        "open-application-submenus-sideways",
        settings.open_application_submenus_sideways,
    );
    set_boolean_if_present(
        &gsettings,
        "show-place-bookmarks",
        settings.show_place_bookmarks,
    );
    set_boolean_if_present(
        &gsettings,
        "show-place-volumes",
        settings.show_place_volumes,
    );
    Ok(())
}
