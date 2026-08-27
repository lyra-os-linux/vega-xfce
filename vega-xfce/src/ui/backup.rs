use std::{cell::RefCell, rc::Rc};

use crate::i18n::gettext;
use adw::prelude::*;

use lyra_vega_dbus::{BackupConfig, BackupSnapshot};

#[derive(Clone)]
pub struct BackupPage {
    pub root: gtk::Widget,
    pub configs: gtk::ListBox,
    pub status: gtk::Label,
    pub run_now: gtk::Button,
    pub new_config: gtk::Button,
    pub delete_config: gtk::Button,
    pub snapshots: gtk::ListBox,
    pub snapshot_status: gtk::Label,
    pub snapshot_paths: gtk::Label,
    pub paths_list: gtk::ListBox,
    pub restore_target: gtk::Entry,
    pub restore_mode: gtk::DropDown,
    pub restore_selected: gtk::Button,
    pub progress_panel: gtk::Box,
    pub progress_label: gtk::Label,
    pub progress: gtk::ProgressBar,
    items: Rc<RefCell<Vec<BackupConfig>>>,
    snapshot_items: Rc<RefCell<Vec<BackupSnapshot>>>,
    path_checks: Rc<RefCell<Vec<(String, gtk::CheckButton)>>>,
}

impl BackupPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando configurações…"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let configs = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        configs.add_css_class("backup-selection");
        let run_now = gtk::Button::builder()
            .label(gettext("Executar agora"))
            .halign(gtk::Align::Start)
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();
        let new_config = gtk::Button::builder()
            .label(gettext("Nova configuração"))
            .css_classes(["suggested-action"])
            .build();
        let delete_config = gtk::Button::builder()
            .label(gettext("Excluir"))
            .sensitive(false)
            .css_classes(["destructive-action"])
            .build();
        let config_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        config_actions.append(&new_config);
        config_actions.append(&run_now);
        config_actions.append(&delete_config);
        let snapshot_status = gtk::Label::builder()
            .label(gettext("Selecione uma configuração para ver os snapshots"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let snapshots = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        snapshots.add_css_class("backup-selection");
        let snapshot_paths = gtk::Label::builder()
            .label(gettext(
                "Os caminhos do snapshot selecionado aparecerão aqui.",
            ))
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .css_classes(["dim-label", "snapshot-paths"])
            .build();
        let paths_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        let restore_target = gtk::Entry::builder()
            .placeholder_text(gettext("Pasta de destino da restauração"))
            .hexpand(true)
            .build();
        let restore_mode = gtk::DropDown::from_strings(&[
            &gettext("Pasta separada (recomendado)"),
            &gettext("Substituir arquivos existentes"),
        ]);
        let restore_selected = gtk::Button::builder()
            .label(gettext("Restaurar selecionados"))
            .halign(gtk::Align::Start)
            .sensitive(false)
            .build();
        let restore_controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
        restore_controls.add_css_class("card");
        restore_controls.append(
            &gtk::Label::builder()
                .label(gettext("Restauração parcial"))
                .xalign(0.0)
                .css_classes(["title-3"])
                .build(),
        );
        restore_controls.append(&restore_target);
        restore_controls.append(&restore_mode);
        restore_controls.append(&restore_selected);
        let progress_label = gtk::Label::builder()
            .label(gettext("Preparando backup…"))
            .xalign(0.0)
            .wrap(true)
            .build();
        let progress = gtk::ProgressBar::builder().show_text(true).build();
        let progress_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        progress_panel.add_css_class("card");
        progress_panel.set_visible(false);
        progress_panel.append(&progress_label);
        progress_panel.append(&progress);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.append(&status);
        content.append(&configs);
        content.append(&config_actions);
        content.append(
            &gtk::Label::builder()
                .label(gettext("Snapshots"))
                .xalign(0.0)
                .css_classes(["title-2"])
                .build(),
        );
        content.append(&snapshot_status);
        content.append(&snapshots);
        content.append(&snapshot_paths);
        content.append(&paths_list);
        content.append(&restore_controls);
        content.append(&progress_panel);

        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        let page = Self {
            root,
            configs,
            status,
            run_now,
            new_config,
            delete_config,
            snapshots,
            snapshot_status,
            snapshot_paths,
            paths_list,
            restore_target,
            restore_mode,
            restore_selected,
            progress_panel,
            progress_label,
            progress,
            items: Rc::new(RefCell::new(Vec::new())),
            snapshot_items: Rc::new(RefCell::new(Vec::new())),
            path_checks: Rc::new(RefCell::new(Vec::new())),
        };
        let button = page.run_now.clone();
        let delete = page.delete_config.clone();
        page.configs.connect_row_selected(move |_, row| {
            button.set_sensitive(row.is_some());
            delete.set_sensitive(row.is_some());
        });
        page
    }

    pub fn show_configs(&self, configs: Vec<BackupConfig>) {
        while let Some(child) = self.configs.first_child() {
            self.configs.remove(&child);
        }
        self.status.set_label(&if configs.is_empty() {
            gettext("Nenhuma configuração de backup encontrada")
        } else {
            gettext("Selecione uma configuração para executar")
        });
        for config in &configs {
            let destination = if config.destination_uuid.is_empty() {
                config.destination.clone()
            } else {
                format!("{} • UUID {}", config.destination, config.destination_uuid)
            };
            let safe_id = gtk::glib::markup_escape_text(&config.id);
            let safe_subtitle = gtk::glib::markup_escape_text(
                &gettext("{destination} • {count} caminho(s) • {frequency}")
                    .replace("{destination}", &destination)
                    .replace("{count}", &config.paths.len().to_string())
                    .replace("{frequency}", &config.frequency),
            );
            self.configs.append(
                &adw::ActionRow::builder()
                    .title(safe_id)
                    .subtitle(safe_subtitle)
                    .activatable(true)
                    .build(),
            );
        }
        *self.items.borrow_mut() = configs;
        self.show_snapshots(Vec::new());
    }

    pub fn selected_config(&self) -> Option<BackupConfig> {
        let index = self.configs.selected_row()?.index() as usize;
        self.items.borrow().get(index).cloned()
    }

    pub fn show_snapshots(&self, snapshots: Vec<BackupSnapshot>) {
        while let Some(child) = self.snapshots.first_child() {
            self.snapshots.remove(&child);
        }
        self.snapshot_status.set_label(&if snapshots.is_empty() {
            gettext("Nenhum snapshot encontrado")
        } else {
            gettext("Selecione um snapshot para inspecionar seus caminhos")
        });
        self.snapshot_paths.set_label(&gettext(
            "Os caminhos do snapshot selecionado aparecerão aqui.",
        ));
        for snapshot in &snapshots {
            let title = format_timestamp(snapshot.timestamp);
            let subtitle = gettext("{count} arquivo(s) • {size}")
                .replace("{count}", &snapshot.file_count.to_string())
                .replace("{size}", &format_bytes(snapshot.size_bytes));
            self.snapshots.append(
                &adw::ActionRow::builder()
                    .title(gtk::glib::markup_escape_text(&title))
                    .subtitle(gtk::glib::markup_escape_text(&subtitle))
                    .activatable(true)
                    .build(),
            );
        }
        *self.snapshot_items.borrow_mut() = snapshots;
    }

    pub fn selected_snapshot(&self) -> Option<BackupSnapshot> {
        let index = self.snapshots.selected_row()?.index() as usize;
        self.snapshot_items.borrow().get(index).cloned()
    }

    pub fn show_snapshot_paths(&self, paths: &[String]) {
        while let Some(child) = self.paths_list.first_child() {
            self.paths_list.remove(&child);
        }
        self.path_checks.borrow_mut().clear();
        self.restore_selected.set_sensitive(false);
        if paths.is_empty() {
            self.snapshot_paths
                .set_label(&gettext("O snapshot não contém caminhos restauráveis."));
            return;
        }
        let visible = paths.iter().take(100).cloned().collect::<Vec<_>>();
        let remaining = paths.len().saturating_sub(visible.len());
        let mut text = gettext("{count} caminho(s) disponível(is)")
            .replace("{count}", &paths.len().to_string());
        if remaining > 0 {
            text.push_str(
                &gettext(" • exibindo 100, mais {remaining}")
                    .replace("{remaining}", &remaining.to_string()),
            );
        }
        self.snapshot_paths.set_label(&text);
        for path in visible {
            let check = gtk::CheckButton::builder()
                .label(&path)
                .halign(gtk::Align::Fill)
                .build();
            let button = self.restore_selected.clone();
            let checks = self.path_checks.clone();
            check.connect_toggled(move |_| {
                button.set_sensitive(checks.borrow().iter().any(|(_, check)| check.is_active()));
            });
            self.paths_list.append(&check);
            self.path_checks.borrow_mut().push((path, check));
        }
    }

    pub fn selected_paths(&self) -> Vec<String> {
        self.path_checks
            .borrow()
            .iter()
            .filter(|(_, check)| check.is_active())
            .map(|(path, _)| path.clone())
            .collect()
    }

    pub fn restore_mode_value(&self) -> &'static str {
        if self.restore_mode.selected() == 1 {
            "replace"
        } else {
            "separate-folder"
        }
    }

    pub fn begin(&self, message: &str) {
        self.progress_panel.set_visible(true);
        self.progress_label.set_label(message);
        self.progress.set_fraction(0.0);
        self.progress.set_text(Some("0%"));
    }

    pub fn update_progress(&self, percent: u32, message: &str) {
        let percent = percent.min(100);
        self.progress_label.set_label(message);
        self.progress.set_fraction(f64::from(percent) / 100.0);
        self.progress.set_text(Some(&format!("{percent}%")));
    }

    pub fn finish(&self, success: bool, message: &str) {
        self.progress_label.set_label(message);
        self.progress.set_fraction(1.0);
        self.progress.set_text(Some(&if success {
            gettext("Concluído")
        } else {
            gettext("Falhou")
        }));
        self.progress_panel
            .remove_css_class(if success { "error" } else { "success" });
        self.progress_panel
            .add_css_class(if success { "success" } else { "error" });
    }
}

fn format_timestamp(timestamp: i64) -> String {
    gtk::glib::DateTime::from_unix_local(timestamp)
        .and_then(|date| date.format("%d/%m/%Y %H:%M"))
        .map(|value| value.to_string())
        .unwrap_or_else(|_| {
            gettext("Snapshot {timestamp}").replace("{timestamp}", &timestamp.to_string())
        })
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn backup_sizes_use_readable_binary_units() {
        assert_eq!(format_bytes(900), "900 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
    }
}
