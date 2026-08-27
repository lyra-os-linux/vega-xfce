use std::{cell::RefCell, rc::Rc};

use crate::i18n::gettext;
use adw::prelude::*;

use lyra_vega_dbus::Snapshot;

type SnapshotCallback = Rc<RefCell<Option<Box<dyn Fn(Snapshot, gtk::Button)>>>>;

#[derive(Clone)]
pub struct SnapshotsPage {
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub list: gtk::ListBox,
    pub create: gtk::Button,
    pub clear: gtk::Button,
    pub clear_progress: gtk::ProgressBar,
    pub retention: gtk::SpinButton,
    pub apply_retention: gtk::Button,
    pub comparison: gtk::Label,
    on_apply: SnapshotCallback,
    on_delete: SnapshotCallback,
    on_compare: SnapshotCallback,
}

impl SnapshotsPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Verificando suporte a snapshots…"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        list.add_css_class("snapshot-selection");
        let create = gtk::Button::builder()
            .label(gettext("Criar ponto"))
            .css_classes(["suggested-action"])
            .build();
        let clear = gtk::Button::builder()
            .label(gettext("Limpar pontos criados"))
            .css_classes(["destructive-action"])
            .sensitive(false)
            .build();
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.append(&create);
        actions.append(&clear);
        let clear_progress = gtk::ProgressBar::builder()
            .text(gettext("Excluindo pontos de restauração…"))
            .show_text(true)
            .visible(false)
            .build();
        let comparison = gtk::Label::builder()
            .label(gettext(
                "Selecione um ponto para comparar os pacotes com o estado atual.",
            ))
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .css_classes(["snapshot-paths"])
            .build();
        let retention = gtk::SpinButton::with_range(1.0, 100.0, 1.0);
        retention.set_value(10.0);
        let apply_retention = gtk::Button::with_label(&gettext("Aplicar retenção"));
        let retention_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        retention_row.add_css_class("card");
        retention_row.append(
            &gtk::Label::builder()
                .label(gettext("Manter os snapshots mais recentes"))
                .xalign(0.0)
                .hexpand(true)
                .build(),
        );
        retention_row.append(&retention);
        retention_row.append(&apply_retention);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.append(&status);
        content.append(&actions);
        content.append(&clear_progress);
        content.append(&list);
        content.append(&comparison);
        content.append(&retention_row);
        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        Self {
            root,
            status,
            list,
            create,
            clear,
            clear_progress,
            retention,
            apply_retention,
            comparison,
            on_apply: Rc::new(RefCell::new(None)),
            on_delete: Rc::new(RefCell::new(None)),
            on_compare: Rc::new(RefCell::new(None)),
        }
    }

    pub fn connect_apply<F: Fn(Snapshot, gtk::Button) + 'static>(&self, callback: F) {
        *self.on_apply.borrow_mut() = Some(Box::new(callback));
    }

    pub fn connect_delete<F: Fn(Snapshot, gtk::Button) + 'static>(&self, callback: F) {
        *self.on_delete.borrow_mut() = Some(Box::new(callback));
    }

    pub fn connect_compare<F: Fn(Snapshot, gtk::Button) + 'static>(&self, callback: F) {
        *self.on_compare.borrow_mut() = Some(Box::new(callback));
    }

    pub fn set_available(&self, available: bool) {
        self.create.set_sensitive(available);
        if !available {
            self.clear.set_sensitive(false);
        }
        if !available {
            self.status
                .set_label(&gettext("Snapshots não são suportados neste sistema."));
        }
    }

    pub fn show_snapshots(&self, snapshots: Vec<Snapshot>) {
        let snapshots = snapshots
            .into_iter()
            .filter(|snapshot| snapshot.id != 0)
            .collect::<Vec<_>>();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.status.set_label(&if snapshots.is_empty() {
            gettext("Nenhum ponto de restauração encontrado")
        } else {
            gettext("Selecione um ponto para comparar, aplicar ou excluir")
        });
        self.clear.set_sensitive(!snapshots.is_empty());
        for snapshot in &snapshots {
            let date = format_timestamp(snapshot.timestamp);
            let title = if snapshot.description.is_empty() {
                gettext("Snapshot #{id}").replace("{id}", &snapshot.id.to_string())
            } else {
                snapshot.description.clone()
            };
            let subtitle = format!("#{0} • {date} • {1}", snapshot.id, snapshot.trigger);
            let row = adw::ActionRow::builder()
                .title(gtk::glib::markup_escape_text(&title))
                .subtitle(gtk::glib::markup_escape_text(&subtitle))
                .activatable(true)
                .build();

            let compare = gtk::Button::builder()
                .label(gettext("Comparar"))
                .valign(gtk::Align::Center)
                .build();
            let on_compare = self.on_compare.clone();
            let compare_snapshot = snapshot.clone();
            compare.connect_clicked(move |button| {
                if let Some(callback) = on_compare.borrow().as_ref() {
                    callback(compare_snapshot.clone(), button.clone());
                }
            });

            let apply = gtk::Button::builder()
                .label(gettext("Aplicar"))
                .valign(gtk::Align::Center)
                .css_classes(["suggested-action"])
                .build();
            let on_apply = self.on_apply.clone();
            let apply_snapshot = snapshot.clone();
            apply.connect_clicked(move |button| {
                if let Some(callback) = on_apply.borrow().as_ref() {
                    callback(apply_snapshot.clone(), button.clone());
                }
            });

            let delete = gtk::Button::builder()
                .label(gettext("Excluir"))
                .valign(gtk::Align::Center)
                .css_classes(["destructive-action"])
                .build();
            let on_delete = self.on_delete.clone();
            let delete_snapshot = snapshot.clone();
            delete.connect_clicked(move |button| {
                if let Some(callback) = on_delete.borrow().as_ref() {
                    callback(delete_snapshot.clone(), button.clone());
                }
            });

            row.add_suffix(&compare);
            row.add_suffix(&apply);
            row.add_suffix(&delete);
            self.list.append(&row);
        }
    }

    pub fn show_comparison(&self, changes: &[String]) {
        let text = if changes.is_empty() {
            gettext("Nenhuma diferença de pacotes em relação ao estado atual.")
        } else {
            changes.join("\n")
        };
        self.comparison.set_label(&text);
    }
}

fn format_timestamp(timestamp: i64) -> String {
    gtk::glib::DateTime::from_unix_local(timestamp)
        .and_then(|date| date.format("%d/%m/%Y %H:%M"))
        .map(|value| value.to_string())
        .unwrap_or_else(|_| timestamp.to_string())
}
