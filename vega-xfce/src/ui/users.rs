use std::{cell::RefCell, rc::Rc};

use crate::i18n::gettext;
use adw::prelude::*;

use lyra_vega_dbus::UserInfo;

type UserRowActions = (gtk::Button, gtk::Button, gtk::Button, bool);

#[derive(Clone)]
pub struct UsersPage {
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub username: gtk::Entry,
    pub full_name: gtk::Entry,
    pub password: gtk::PasswordEntry,
    pub password_confirm: gtk::PasswordEntry,
    pub groups: gtk::Entry,
    pub photo: gtk::Button,
    pub photo_data: Rc<RefCell<Vec<u8>>>,
    pub admin: gtk::CheckButton,
    pub open_create: gtk::Button,
    pub create: gtk::Button,
    pub editor_dialog: adw::Dialog,
    pub editor_title: gtk::Label,
    pub list: gtk::ListBox,
    pub edit: gtk::Button,
    pub change_admin: gtk::Button,
    pub remove: gtk::Button,
    pub editing: Rc<RefCell<Option<String>>>,
    items: Rc<RefCell<Vec<UserInfo>>>,
    row_actions: Rc<RefCell<Vec<UserRowActions>>>,
}

impl UsersPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando usuários…"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let username = gtk::Entry::builder()
            .placeholder_text(gettext("nome de usuário"))
            .hexpand(true)
            .build();
        let full_name = gtk::Entry::builder()
            .placeholder_text(gettext("Nome completo"))
            .hexpand(true)
            .build();
        let password = gtk::PasswordEntry::builder()
            .placeholder_text(gettext("Senha (mínimo de 8 caracteres)"))
            .show_peek_icon(true)
            .build();
        let password_confirm = gtk::PasswordEntry::builder()
            .placeholder_text(gettext("Confirmar senha"))
            .show_peek_icon(true)
            .build();
        let groups = gtk::Entry::builder()
            .placeholder_text(gettext("Grupos adicionais, separados por vírgula"))
            .build();
        let photo = gtk::Button::builder()
            .label(gettext("Selecionar foto…"))
            .halign(gtk::Align::Start)
            .build();
        let photo_data = Rc::new(RefCell::new(Vec::new()));
        let admin = gtk::CheckButton::builder()
            .label(gettext("Administrador"))
            .active(true)
            .build();
        let create = gtk::Button::builder()
            .label(gettext("Criar usuário"))
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();
        let open_create = gtk::Button::builder()
            .label(gettext("Novo usuário"))
            .css_classes(["suggested-action"])
            .build();
        let form_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        form_row.append(&admin);
        form_row.append(&photo);
        form_row.append(&create);
        let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
        form.add_css_class("card");
        form.append(&full_name);
        form.append(&username);
        form.append(&password);
        form.append(&password_confirm);
        form.append(&groups);
        form.append(
            &gtk::Label::builder()
                .label(gettext("Informe grupos existentes, por exemplo: audio, video, docker. O grupo wheel é incluído automaticamente para administradores."))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
        form.append(&form_row);
        let editor_title = gtk::Label::builder()
            .label(gettext("Novo usuário"))
            .css_classes(["heading"])
            .build();
        let editor_header = adw::HeaderBar::builder()
            .title_widget(&editor_title)
            .show_end_title_buttons(true)
            .build();
        let editor_scroll = gtk::ScrolledWindow::builder()
            .child(&form)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        let editor_layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor_layout.append(&editor_header);
        editor_layout.append(&editor_scroll);
        let editor_dialog = adw::Dialog::builder()
            .child(&editor_layout)
            .content_width(620)
            .content_height(560)
            .build();

        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        let change_admin = action(&gettext("Alterar administração"));
        let edit = action(&gettext("Editar"));
        let remove = gtk::Button::builder()
            .label(gettext("Remover"))
            .sensitive(false)
            .css_classes(["destructive-action"])
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.add_css_class("compact-page");
        content.append(
            &gtk::Label::builder()
                .label(gettext("Contas e Usuários"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(
            &gtk::Label::builder()
                .label(gettext("Criação, remoção e controle de administração"))
                .xalign(0.0)
                .css_classes(["dim-label"])
                .build(),
        );
        let list_header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        list_header.append(
            &gtk::Label::builder()
                .label(gettext("Usuários do sistema"))
                .xalign(0.0)
                .hexpand(true)
                .css_classes(["title-2"])
                .build(),
        );
        list_header.append(&open_create);
        content.append(&status);
        content.append(&list_header);
        content.append(&list);
        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        let page = Self {
            root,
            status,
            username,
            full_name,
            password,
            password_confirm,
            groups,
            photo,
            photo_data,
            admin,
            open_create,
            create,
            editor_dialog,
            editor_title,
            list,
            edit,
            change_admin,
            remove,
            editing: Rc::new(RefCell::new(None)),
            items: Rc::new(RefCell::new(Vec::new())),
            row_actions: Rc::new(RefCell::new(Vec::new())),
        };
        for entry in [&page.username, &page.full_name] {
            let entry_page = page.clone();
            entry.connect_changed(move |_| entry_page.update_create_sensitivity());
        }
        for entry in [&page.password, &page.password_confirm] {
            let entry_page = page.clone();
            entry.connect_changed(move |_| entry_page.update_create_sensitivity());
        }
        let selection_page = page.clone();
        page.list
            .connect_row_selected(move |_, _| selection_page.update_actions());
        page
    }

    pub fn show(&self, items: Vec<UserInfo>) {
        self.row_actions.borrow_mut().clear();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        if items.is_empty() {
            self.list.set_selection_mode(gtk::SelectionMode::None);
            self.list.append(
                &adw::ActionRow::builder()
                    .title(gettext("Nenhum usuário listado"))
                    .subtitle(gettext("Ainda não há contas cadastradas."))
                    .build(),
            );
        } else {
            self.list.set_selection_mode(gtk::SelectionMode::Single);
            for user in &items {
                let row = adw::ActionRow::builder()
                    .title(gtk::glib::markup_escape_text(&user.username))
                    .subtitle(if user.full_name.is_empty() {
                        if user.is_admin {
                            gettext("Administrador")
                        } else {
                            gettext("Usuário comum")
                        }
                    } else {
                        gettext("{name} — {role}")
                            .replace("{name}", &user.full_name)
                            .replace(
                                "{role}",
                                &if user.is_admin {
                                    gettext("Administrador")
                                } else {
                                    gettext("Usuário comum")
                                },
                            )
                    })
                    .activatable(true)
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name("avatar-default-symbolic"));
                let mutable = user.username != "root";
                let edit_button = gtk::Button::builder()
                    .label(gettext("Editar"))
                    .sensitive(mutable)
                    .valign(gtk::Align::Center)
                    .css_classes(["compact"])
                    .build();
                let admin_button = gtk::Button::builder()
                    .label(if user.is_admin {
                        gettext("Remover admin")
                    } else {
                        gettext("Tornar admin")
                    })
                    .sensitive(mutable)
                    .valign(gtk::Align::Center)
                    .css_classes(["compact"])
                    .build();
                let remove_button = gtk::Button::builder()
                    .label(gettext("Remover"))
                    .sensitive(mutable)
                    .valign(gtk::Align::Center)
                    .css_classes(["destructive-action", "compact"])
                    .build();
                row.add_suffix(&edit_button);
                row.add_suffix(&admin_button);
                row.add_suffix(&remove_button);
                self.list.append(&row);
                let admin_list = self.list.clone();
                let admin_row = row.clone();
                let admin_action = self.change_admin.clone();
                admin_button.connect_clicked(move |_| {
                    admin_list.select_row(Some(&admin_row));
                    admin_action.emit_clicked();
                });
                let edit_list = self.list.clone();
                let edit_row = row.clone();
                let edit_action = self.edit.clone();
                edit_button.connect_clicked(move |_| {
                    edit_list.select_row(Some(&edit_row));
                    edit_action.emit_clicked();
                });
                let remove_list = self.list.clone();
                let remove_row = row.clone();
                let remove_action = self.remove.clone();
                remove_button.connect_clicked(move |_| {
                    remove_list.select_row(Some(&remove_row));
                    remove_action.emit_clicked();
                });
                self.row_actions.borrow_mut().push((
                    edit_button,
                    admin_button,
                    remove_button,
                    mutable,
                ));
            }
        }
        self.status
            .set_label(&gettext("{count} usuário(s)").replace("{count}", &items.len().to_string()));
        *self.items.borrow_mut() = items;
        self.update_actions();
    }

    pub fn selected(&self) -> Option<UserInfo> {
        self.items
            .borrow()
            .get(self.list.selected_row()?.index() as usize)
            .cloned()
    }

    pub fn set_busy(&self, busy: bool) {
        self.username
            .set_sensitive(!busy && self.editing.borrow().is_none());
        self.full_name.set_sensitive(!busy);
        self.password.set_sensitive(!busy);
        self.password_confirm.set_sensitive(!busy);
        self.groups.set_sensitive(!busy);
        self.photo.set_sensitive(!busy);
        self.admin.set_sensitive(!busy);
        self.create.set_sensitive(!busy && self.valid_form());
        for (edit, admin, remove, mutable) in self.row_actions.borrow().iter() {
            edit.set_sensitive(!busy && *mutable);
            admin.set_sensitive(!busy && *mutable);
            remove.set_sensitive(!busy && *mutable);
        }
        if busy {
            self.edit.set_sensitive(false);
            self.change_admin.set_sensitive(false);
            self.remove.set_sensitive(false);
        } else {
            self.update_actions();
        }
    }

    fn valid_form(&self) -> bool {
        let password_valid = if self.editing.borrow().is_some() && self.password.text().is_empty() {
            true
        } else {
            self.password.text().chars().count() >= 8
                && self.password.text() == self.password_confirm.text()
        };
        valid_username(self.username.text().as_str())
            && !self.full_name.text().trim().is_empty()
            && password_valid
    }

    fn update_create_sensitivity(&self) {
        self.create.set_sensitive(self.valid_form());
    }

    fn update_actions(&self) {
        let selected = self.selected();
        let mutable = selected
            .as_ref()
            .is_some_and(|user| user.username != "root");
        self.edit.set_sensitive(mutable);
        self.change_admin.set_sensitive(mutable);
        self.remove.set_sensitive(mutable);
        if let Some(user) = selected {
            self.change_admin.set_label(&if user.is_admin {
                gettext("Remover admin")
            } else {
                gettext("Tornar admin")
            });
        }
    }

    pub fn begin_edit(&self, user: &UserInfo) {
        *self.editing.borrow_mut() = Some(user.username.clone());
        self.username.set_text(&user.username);
        self.username.set_sensitive(false);
        self.full_name.set_text(&user.full_name);
        self.groups.set_text(&user.groups.join(", "));
        self.admin.set_active(user.is_admin);
        self.password.set_text("");
        self.password_confirm.set_text("");
        self.photo_data.borrow_mut().clear();
        self.photo
            .set_label(&gettext("Manter foto atual (ou selecionar outra)…"));
        self.create.set_label(&gettext("Salvar alterações"));
        self.editor_title.set_label(&gettext("Editar usuário"));
        self.update_create_sensitivity();
        self.full_name.grab_focus();
        self.editor_dialog.present(Some(&self.root));
    }

    pub fn finish_edit(&self) {
        *self.editing.borrow_mut() = None;
        self.username.set_sensitive(true);
        self.create.set_label(&gettext("Criar usuário"));
        self.editor_title.set_label(&gettext("Novo usuário"));
    }

    pub fn begin_create(&self) {
        self.finish_edit();
        self.username.set_text("");
        self.full_name.set_text("");
        self.password.set_text("");
        self.password_confirm.set_text("");
        self.groups.set_text("");
        self.admin.set_active(true);
        self.photo_data.borrow_mut().clear();
        self.photo.set_label(&gettext("Selecionar foto…"));
        self.update_create_sensitivity();
        self.editor_dialog.present(Some(&self.root));
        self.full_name.grab_focus();
    }
}

fn action(label: &str) -> gtk::Button {
    gtk::Button::builder().label(label).sensitive(false).build()
}

fn valid_username(value: &str) -> bool {
    let value = value.trim();
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first == '_')
        && chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-')
        })
}

#[cfg(test)]
mod tests {
    use super::valid_username;

    #[test]
    fn username_validation_matches_backend_rules_for_regular_accounts() {
        for valid in ["rodrigo", "user_2", "dev-user"] {
            assert!(valid_username(valid));
        }
        for invalid in ["", "User", "2user", "user name", "user!"] {
            assert!(!valid_username(invalid));
        }
    }
}
