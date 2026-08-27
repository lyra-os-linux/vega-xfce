use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::i18n::gettext;
use adw::prelude::*;

use lyra_vega_dbus::ManagedService;

struct ServiceRowActions {
    enable: gtk::Button,
    running: gtk::Button,
    restart: gtk::Button,
    service: ManagedService,
}

#[derive(Clone)]
pub struct ServicesPage {
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub list: gtk::ListBox,
    pub curated: gtk::ToggleButton,
    pub all: gtk::ToggleButton,
    pub enable: gtk::Button,
    pub running: gtk::Button,
    pub restart: gtk::Button,
    items: Rc<RefCell<Vec<ManagedService>>>,
    row_actions: Rc<RefCell<Vec<ServiceRowActions>>>,
    busy: Rc<Cell<bool>>,
}

impl ServicesPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando serviços…"))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();
        let curated = gtk::ToggleButton::with_label(&gettext("Principais"));
        curated.set_active(true);
        curated.add_css_class("module-tab");
        let all = gtk::ToggleButton::with_label(&gettext("Todos"));
        all.set_group(Some(&curated));
        all.add_css_class("module-tab");
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabs.add_css_class("module-tabs");
        tabs.append(&curated);
        tabs.append(&all);
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list", "services-list"])
            .build();
        let enable = action(&gettext("Habilitar"));
        let running = action(&gettext("Iniciar"));
        let restart = action(&gettext("Reiniciar"));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.add_css_class("compact-page");
        content.append(
            &gtk::Label::builder()
                .label(gettext("Serviços"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(
            &gtk::Label::builder()
                .label(gettext("Units do systemd gerenciadas pelo vegad"))
                .xalign(0.0)
                .css_classes(["dim-label"])
                .build(),
        );
        content.append(&tabs);
        content.append(&status);
        content.append(&list);
        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        let page = Self {
            root,
            status,
            list,
            curated,
            all,
            enable,
            running,
            restart,
            items: Rc::new(RefCell::new(Vec::new())),
            row_actions: Rc::new(RefCell::new(Vec::new())),
            busy: Rc::new(Cell::new(false)),
        };
        let selection = page.clone();
        page.list
            .connect_row_selected(move |_, _| selection.update_actions());
        page
    }

    pub fn show(&self, items: Vec<ManagedService>) {
        self.busy.set(false);
        self.row_actions.borrow_mut().clear();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        for service in &items {
            let row = adw::ActionRow::builder()
                .title(gtk::glib::markup_escape_text(&service.label))
                .subtitle(gtk::glib::markup_escape_text(&format!(
                    "{} • {}",
                    service.name, service.description
                )))
                .title_lines(1)
                .subtitle_lines(1)
                .activatable(true)
                .build();
            row.add_suffix(&gtk::Label::new(Some(&format!(
                "{} • {}",
                if service.active {
                    gettext("Ativo")
                } else {
                    gettext("Parado")
                },
                if service.enabled {
                    gettext("Habilitado")
                } else {
                    gettext("Desabilitado")
                }
            ))));
            let enable_button = row_action(&if service.enabled {
                gettext("Desabilitar")
            } else {
                gettext("Habilitar")
            });
            let running_button = row_action(&if service.active {
                gettext("Parar")
            } else {
                gettext("Iniciar")
            });
            let restart_button = row_action(&gettext("Reiniciar"));
            enable_button.set_sensitive(service.available);
            running_button.set_sensitive(service.available);
            restart_button.set_sensitive(service.available && service.active);
            row.add_suffix(&enable_button);
            row.add_suffix(&running_button);
            row.add_suffix(&restart_button);
            self.list.append(&row);
            connect_row_action(&enable_button, &self.list, &row, &self.enable);
            connect_row_action(&running_button, &self.list, &row, &self.running);
            connect_row_action(&restart_button, &self.list, &row, &self.restart);
            self.row_actions.borrow_mut().push(ServiceRowActions {
                enable: enable_button,
                running: running_button,
                restart: restart_button,
                service: service.clone(),
            });
        }
        self.status
            .set_label(&gettext("{count} serviço(s)").replace("{count}", &items.len().to_string()));
        *self.items.borrow_mut() = items;
        self.update_actions();
    }

    pub fn selected(&self) -> Option<ManagedService> {
        self.items
            .borrow()
            .get(self.list.selected_row()?.index() as usize)
            .cloned()
    }

    pub fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        for row in self.row_actions.borrow().iter() {
            row.enable.set_sensitive(!busy && row.service.available);
            row.running.set_sensitive(!busy && row.service.available);
            row.restart
                .set_sensitive(!busy && row.service.available && row.service.active);
        }
        self.update_actions();
    }

    fn update_actions(&self) {
        if self.busy.get() {
            for button in [&self.enable, &self.running, &self.restart] {
                button.set_sensitive(false);
            }
            return;
        }
        let Some(service) = self.selected() else {
            for b in [&self.enable, &self.running, &self.restart] {
                b.set_sensitive(false);
            }
            return;
        };
        self.enable.set_sensitive(service.available);
        self.running.set_sensitive(service.available);
        self.restart
            .set_sensitive(service.available && service.active);
        self.enable.set_label(&if service.enabled {
            gettext("Desabilitar")
        } else {
            gettext("Habilitar")
        });
        self.running.set_label(&if service.active {
            gettext("Parar")
        } else {
            gettext("Iniciar")
        });
    }
}

fn action(label: &str) -> gtk::Button {
    gtk::Button::builder().label(label).sensitive(false).build()
}

fn row_action(label: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .valign(gtk::Align::Center)
        .css_classes(["compact"])
        .build()
}

fn connect_row_action(
    button: &gtk::Button,
    list: &gtk::ListBox,
    row: &adw::ActionRow,
    action: &gtk::Button,
) {
    let list = list.clone();
    let row = row.clone();
    let action = action.clone();
    button.connect_clicked(move |_| {
        list.select_row(Some(&row));
        action.emit_clicked();
    });
}
