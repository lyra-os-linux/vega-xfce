use crate::i18n::gettext;
use adw::prelude::*;

use lyra_vega_dbus::DateTimeStatus;

#[derive(Clone)]
pub struct DateTimePage {
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub timezone: gtk::DropDown,
    pub ntp: gtk::Switch,
    pub locale: gtk::DropDown,
    pub keymap: gtk::DropDown,
    pub apply: gtk::Button,
}

impl DateTimePage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando data, hora e idioma…"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let timezone = gtk::DropDown::from_strings(&[]);
        let ntp = gtk::Switch::new();
        ntp.set_halign(gtk::Align::End);
        ntp.set_valign(gtk::Align::Center);
        ntp.add_css_class("compact-switch");
        let locale = gtk::DropDown::from_strings(&[]);
        let keymap = gtk::DropDown::from_strings(&[]);
        for dropdown in [&timezone, &locale, &keymap] {
            dropdown.set_size_request(300, -1);
            dropdown.set_halign(gtk::Align::End);
            dropdown.set_valign(gtk::Align::Center);
        }
        let apply = gtk::Button::builder()
            .label(gettext("Aplicar ao sistema"))
            .halign(gtk::Align::End)
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();

        let settings = adw::PreferencesGroup::builder()
            .title(gettext("Configuração global"))
            .description(gettext("As alterações afetam todos os usuários"))
            .build();
        settings.add(&property(&gettext("Fuso horário"), &timezone));
        settings.add(&property(&gettext("Sincronização automática (NTP)"), &ntp));
        settings.add(&property(&gettext("Idioma e formato regional"), &locale));
        settings.add(&property(&gettext("Layout do teclado"), &keymap));

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.add_css_class("compact-page");
        content.append(
            &gtk::Label::builder()
                .label(gettext("Data, Hora e Idioma"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(
            &gtk::Label::builder()
                .label(gettext("Timezone, NTP, locale e teclado"))
                .xalign(0.0)
                .css_classes(["dim-label"])
                .build(),
        );
        content.append(&status);
        content.append(&settings);
        content.append(&apply);
        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        Self {
            root,
            status,
            timezone,
            ntp,
            locale,
            keymap,
            apply,
        }
    }

    pub fn show(
        &self,
        status: &DateTimeStatus,
        timezones: &[String],
        locales: &[String],
        keymaps: &[String],
    ) {
        set_choices(&self.timezone, timezones, &status.timezone);
        set_choices(&self.locale, locales, &status.locale);
        set_choices(&self.keymap, keymaps, &status.keymap);
        self.ntp.set_active(status.ntp);
        self.apply.set_sensitive(true);
        self.status
            .set_label(&gettext("Configuração atual carregada"));
    }

    pub fn selected(dropdown: &gtk::DropDown) -> String {
        dropdown
            .selected_item()
            .and_downcast::<gtk::StringObject>()
            .map(|item| item.string().to_string())
            .unwrap_or_default()
    }
}

fn set_choices(dropdown: &gtk::DropDown, values: &[String], current: &str) {
    let mut choices = values.to_vec();
    if !current.is_empty() && !choices.iter().any(|value| value == current) {
        choices.insert(0, current.to_owned());
    }
    let strings = choices.iter().map(String::as_str).collect::<Vec<_>>();
    dropdown.set_model(Some(&gtk::StringList::new(&strings)));
    if let Some(index) = choices.iter().position(|value| value == current) {
        dropdown.set_selected(index as u32);
    }
}

fn property(title: &str, widget: &impl IsA<gtk::Widget>) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).build();
    row.add_suffix(widget);
    row
}
