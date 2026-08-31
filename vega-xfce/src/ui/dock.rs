use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::i18n::gettext;
use adw::prelude::*;

use crate::dock::DockSettings;

type ChangeHandler = Rc<dyn Fn(DockSettings)>;

const POSITIONS: &[&str] = &["bottom", "left", "right"];
const HIDE_MODES: &[&str] = &["intelligent", "autohide", "always"];
const MINIMIZE_ANIMATIONS: &[&str] = &["zoom", "fade", "none"];
const CONTENT_ALIGNMENTS: &[&str] = &["start", "center", "end"];
const RUNNING_APPS_POSITIONS: &[&str] = &["start", "end"];

fn position_label(id: &str) -> String {
    match id {
        "left" => gettext("Esquerda"),
        "right" => gettext("Direita"),
        _ => gettext("Inferior"),
    }
}

fn hide_mode_label(id: &str) -> String {
    match id {
        "autohide" => gettext("Auto hide"),
        "always" => gettext("Sempre ativo"),
        _ => gettext("Ocultação inteligente"),
    }
}

fn minimize_animation_label(id: &str) -> String {
    match id {
        "fade" => gettext("Desvanecer"),
        "none" => gettext("Sem animação"),
        _ => gettext("Zoom ao ícone"),
    }
}

fn content_alignment_label(id: &str) -> String {
    match id {
        "start" => gettext("Início"),
        "end" => gettext("Fim"),
        _ => gettext("Centro"),
    }
}

fn running_apps_position_label(id: &str) -> String {
    match id {
        "start" => gettext("Início"),
        _ => gettext("Fim"),
    }
}

fn id_dropdown(ids: &[&str], labels: impl Fn(&str) -> String, current: &str) -> gtk::DropDown {
    let strings: Vec<String> = ids.iter().map(|id| labels(id)).collect();
    let model = gtk::StringList::new(&strings.iter().map(String::as_str).collect::<Vec<_>>());
    let dropdown = gtk::DropDown::builder()
        .model(&model)
        .valign(gtk::Align::Center)
        .build();
    let index = ids.iter().position(|&id| id == current).unwrap_or(0);
    dropdown.set_selected(index as u32);
    dropdown
}

fn dropdown_selected<'a>(dropdown: &gtk::DropDown, ids: &'a [&str]) -> &'a str {
    ids.get(dropdown.selected() as usize)
        .copied()
        .unwrap_or(ids[0])
}

#[derive(Clone)]
pub struct DockPage {
    #[allow(dead_code)]
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub position: gtk::DropDown,
    pub hide_mode: gtk::DropDown,
    pub icon_size: gtk::SpinButton,
    pub edge_margin: gtk::SpinButton,
    pub hide_delay: gtk::SpinButton,
    pub animation: gtk::Switch,
    pub minimize_animation: gtk::DropDown,
    pub extend_to_edges: gtk::Switch,
    pub content_alignment: gtk::DropDown,
    pub show_running: gtk::Switch,
    pub running_apps_position: gtk::DropDown,
    pub show_trash: gtk::Switch,
    pub show_apps_button: gtk::Switch,
    pub fullscreen_hide: gtk::Switch,
    suppress: Rc<Cell<bool>>,
    change_handlers: Rc<RefCell<Vec<ChangeHandler>>>,
}

impl DockPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando configuração…"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();

        let position = id_dropdown(POSITIONS, position_label, "bottom");
        let hide_mode = id_dropdown(HIDE_MODES, hide_mode_label, "intelligent");
        let icon_size = gtk::SpinButton::with_range(24.0, 96.0, 1.0);
        let edge_margin = gtk::SpinButton::with_range(0.0, 48.0, 1.0);
        let hide_delay = gtk::SpinButton::with_range(100.0, 3000.0, 100.0);
        let animation = switch();
        let minimize_animation = id_dropdown(MINIMIZE_ANIMATIONS, minimize_animation_label, "zoom");
        let extend_to_edges = switch();
        let content_alignment = id_dropdown(CONTENT_ALIGNMENTS, content_alignment_label, "center");
        let show_running = switch();
        let running_apps_position =
            id_dropdown(RUNNING_APPS_POSITIONS, running_apps_position_label, "end");
        let show_trash = switch();
        let show_apps_button = switch();
        let fullscreen_hide = switch();

        let appearance_group = adw::PreferencesGroup::builder()
            .title(gettext("Aparência"))
            .build();
        appearance_group.add(&property_row(&gettext("Posição"), &position));
        appearance_group.add(&property_row(&gettext("Tamanho dos ícones"), &icon_size));
        appearance_group.add(&property_row(&gettext("Margem da borda"), &edge_margin));
        appearance_group.add(&property_row(&gettext("Animações"), &animation));
        appearance_group.add(&property_row(
            &gettext("Animação ao minimizar"),
            &minimize_animation,
        ));
        appearance_group.add(&property_row(
            &gettext("Estender até as bordas"),
            &extend_to_edges,
        ));
        appearance_group.add(&property_row(&gettext("Alinhamento"), &content_alignment));

        let behavior_group = adw::PreferencesGroup::builder()
            .title(gettext("Comportamento"))
            .build();
        behavior_group.add(&property_row(&gettext("Visibilidade do dock"), &hide_mode));
        behavior_group.add(&property_row(
            &gettext("Atraso para ocultar (ms)"),
            &hide_delay,
        ));
        behavior_group.add(&property_row(
            &gettext("Ocultar em tela cheia"),
            &fullscreen_hide,
        ));

        let content_group = adw::PreferencesGroup::builder()
            .title(gettext("Elementos exibidos"))
            .build();
        content_group.add(&property_row(
            &gettext("Aplicativos em execução"),
            &show_running,
        ));
        content_group.add(&property_row(
            &gettext("Posição dos apps abertos"),
            &running_apps_position,
        ));
        content_group.add(&property_row(&gettext("Lixeira"), &show_trash));
        content_group.add(&property_row(
            &gettext("Botão de aplicativos"),
            &show_apps_button,
        ));

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.append(&status);
        content.append(&appearance_group);
        content.append(&behavior_group);
        content.append(&content_group);

        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();

        let page = Self {
            root,
            status,
            position,
            hide_mode,
            icon_size,
            edge_margin,
            hide_delay,
            animation,
            minimize_animation,
            extend_to_edges,
            content_alignment,
            show_running,
            running_apps_position,
            show_trash,
            show_apps_button,
            fullscreen_hide,
            suppress: Rc::new(Cell::new(false)),
            change_handlers: Rc::new(RefCell::new(Vec::new())),
        };
        page.wire_changed_signals();
        page
    }

    /// Chamado a cada mudança de controle (exceto durante `show`, quando os
    /// valores são carregados programaticamente) para aplicar de imediato,
    /// no mesmo padrão da aba Aparência — sem botão "Aplicar".
    pub fn connect_changed(&self, handler: impl Fn(DockSettings) + 'static) {
        self.change_handlers.borrow_mut().push(Rc::new(handler));
    }

    fn emit_changed(&self) {
        if self.suppress.get() {
            return;
        }
        let settings = self.selected();
        for handler in self.change_handlers.borrow().iter() {
            handler(settings.clone());
        }
    }

    fn wire_changed_signals(&self) {
        let page = self.clone();
        self.position
            .connect_selected_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.hide_mode
            .connect_selected_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.icon_size
            .connect_value_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.edge_margin
            .connect_value_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.hide_delay
            .connect_value_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.animation
            .connect_active_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.minimize_animation
            .connect_selected_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.extend_to_edges
            .connect_active_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.content_alignment
            .connect_selected_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.show_running
            .connect_active_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.running_apps_position
            .connect_selected_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.show_trash
            .connect_active_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.show_apps_button
            .connect_active_notify(move |_| page.emit_changed());
        let page = self.clone();
        self.fullscreen_hide
            .connect_active_notify(move |_| page.emit_changed());
    }

    pub fn show(&self, settings: &DockSettings) {
        self.suppress.set(true);
        self.position.set_selected(
            POSITIONS
                .iter()
                .position(|&id| id == settings.position)
                .unwrap_or(0) as u32,
        );
        self.hide_mode.set_selected(
            HIDE_MODES
                .iter()
                .position(|&id| id == settings.hide_mode)
                .unwrap_or(0) as u32,
        );
        self.icon_size.set_value(f64::from(settings.icon_size));
        self.edge_margin.set_value(f64::from(settings.edge_margin));
        self.hide_delay.set_value(f64::from(settings.hide_delay_ms));
        self.animation.set_active(settings.animation);
        self.minimize_animation.set_selected(
            MINIMIZE_ANIMATIONS
                .iter()
                .position(|&id| id == settings.minimize_animation)
                .unwrap_or(0) as u32,
        );
        self.extend_to_edges.set_active(settings.extend_to_edges);
        self.content_alignment.set_selected(
            CONTENT_ALIGNMENTS
                .iter()
                .position(|&id| id == settings.content_alignment)
                .unwrap_or(0) as u32,
        );
        self.show_running.set_active(settings.show_running);
        self.running_apps_position.set_selected(
            RUNNING_APPS_POSITIONS
                .iter()
                .position(|&id| id == settings.running_apps_position)
                .unwrap_or(0) as u32,
        );
        self.show_trash.set_active(settings.show_trash);
        self.show_apps_button.set_active(settings.show_apps_button);
        self.fullscreen_hide.set_active(settings.fullscreen_hide);
        self.suppress.set(false);
        self.status
            .set_label(&gettext("Configuração atual carregada"));
    }

    pub fn selected(&self) -> DockSettings {
        DockSettings {
            position: dropdown_selected(&self.position, POSITIONS).to_string(),
            hide_mode: dropdown_selected(&self.hide_mode, HIDE_MODES).to_string(),
            hide_delay_ms: self.hide_delay.value_as_int().max(0) as u32,
            icon_size: self.icon_size.value_as_int().max(0) as u32,
            edge_margin: self.edge_margin.value_as_int().max(0) as u32,
            animation: self.animation.is_active(),
            minimize_animation: dropdown_selected(&self.minimize_animation, MINIMIZE_ANIMATIONS)
                .to_string(),
            extend_to_edges: self.extend_to_edges.is_active(),
            content_alignment: dropdown_selected(&self.content_alignment, CONTENT_ALIGNMENTS)
                .to_string(),
            show_running: self.show_running.is_active(),
            running_apps_position: dropdown_selected(
                &self.running_apps_position,
                RUNNING_APPS_POSITIONS,
            )
            .to_string(),
            show_trash: self.show_trash.is_active(),
            show_apps_button: self.show_apps_button.is_active(),
            fullscreen_hide: self.fullscreen_hide.is_active(),
        }
    }
}

impl Default for DockPage {
    fn default() -> Self {
        Self::new()
    }
}

fn switch() -> gtk::Switch {
    gtk::Switch::builder()
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build()
}

fn property_row(title: &str, widget: &impl IsA<gtk::Widget>) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).build();
    row.add_suffix(widget);
    row
}
