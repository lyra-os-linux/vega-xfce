use gtk::{gio, gio::prelude::*};

const SCHEMA: &str = "org.gnome.desktop.interface";

/// Tema claro/escuro do GNOME inteiro (`color-scheme`), o mesmo valor lido
/// pelo GNOME Shell, Nautilus e qualquer app libadwaita — igual ao painel
/// Aparência das Configurações do GNOME.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// `org.gnome.desktop.interface` é do GNOME, não do vegad — mesma lógica de
/// schema_available() do wallpaper/screensaver, sem depender do backend.
pub fn schema_available() -> bool {
    gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup(SCHEMA, true))
        .is_some()
}

pub fn current_theme() -> Theme {
    if !schema_available() {
        return Theme::default();
    }
    match gio::Settings::new(SCHEMA).string("color-scheme").as_str() {
        "prefer-dark" => Theme::Dark,
        "prefer-light" => Theme::Light,
        _ => Theme::System,
    }
}

pub fn apply_theme(theme: Theme) {
    if !schema_available() {
        return;
    }
    let value = match theme {
        Theme::System => "default",
        Theme::Light => "prefer-light",
        Theme::Dark => "prefer-dark",
    };
    let _ = gio::Settings::new(SCHEMA).set_string("color-scheme", value);
}

/// Tema de ícones padrão da Vega — reaplicado sempre que o usuário troca o
/// card de tema, pra não ficar com um tema de ícones genérico depois de
/// mexer só na claridade da interface.
///
/// O pacote de ícones se chamava "Lyra-Enterprise-Icons" antes do rename pra
/// "Lyra OS" (Lyra-Theme@47d0ff4). Mantenha este nome em sincronia com
/// `Name=` em `Lyra-OS-Icons/index.theme` nesse repositório.
const ICON_THEME_NAME: &str = "Lyra-OS-Icons";

pub fn apply_icon_theme() {
    if !schema_available() {
        return;
    }
    let _ = gio::Settings::new(SCHEMA).set_string("icon-theme", ICON_THEME_NAME);
}
