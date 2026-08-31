use crate::i18n::gettext;
use adw::prelude::*;

use crate::screensaver::ScreensaverSettings;

#[derive(Clone)]
pub struct ScreensaverPage {
    #[allow(dead_code)]
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub lock_enabled: gtk::Switch,
    pub lock_delay: gtk::SpinButton,
    pub idle_delay: gtk::SpinButton,
    pub apply: gtk::Button,
}

impl ScreensaverPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando configuração…"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();

        let lock_enabled = gtk::Switch::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build();
        let lock_delay = gtk::SpinButton::with_range(0.0, 3600.0, 5.0);
        let idle_delay = gtk::SpinButton::with_range(0.0, 7200.0, 30.0);

        let settings_group = adw::PreferencesGroup::builder()
            .title(gettext("Bloqueio de tela"))
            .description(gettext(
                "Essas preferências valem só para a sua sessão, como nas Configurações do GNOME.",
            ))
            .build();
        settings_group.add(&property_row(
            &gettext("Bloquear automaticamente"),
            &lock_enabled,
        ));
        settings_group.add(&property_row(
            &gettext("Bloquear após ficar ocioso (segundos)"),
            &lock_delay,
        ));
        settings_group.add(&property_row(
            &gettext("Suspender tela após inatividade (segundos)"),
            &idle_delay,
        ));

        let apply = gtk::Button::builder()
            .label(gettext("Aplicar"))
            .halign(gtk::Align::End)
            .css_classes(["suggested-action"])
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.append(&status);
        content.append(&settings_group);
        content.append(&apply);

        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();

        Self {
            root,
            status,
            lock_enabled,
            lock_delay,
            idle_delay,
            apply,
        }
    }

    pub fn show(&self, settings: &ScreensaverSettings) {
        self.lock_enabled.set_active(settings.lock_enabled);
        self.lock_delay
            .set_value(f64::from(settings.lock_delay_secs));
        self.idle_delay
            .set_value(f64::from(settings.idle_delay_secs));
        self.status
            .set_label(&gettext("Configuração atual carregada"));
    }

    pub fn selected(&self) -> ScreensaverSettings {
        ScreensaverSettings {
            lock_enabled: self.lock_enabled.is_active(),
            lock_delay_secs: self.lock_delay.value_as_int().max(0) as u32,
            idle_delay_secs: self.idle_delay.value_as_int().max(0) as u32,
        }
    }
}

impl Default for ScreensaverPage {
    fn default() -> Self {
        Self::new()
    }
}

fn property_row(title: &str, widget: &impl IsA<gtk::Widget>) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).build();
    row.add_suffix(widget);
    row
}
