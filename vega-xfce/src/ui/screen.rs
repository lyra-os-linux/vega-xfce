use crate::i18n::gettext;
use adw::prelude::*;

use super::{DockPage, MenuPage, ScreensaverPage, WallpaperPage};
use crate::appearance::Theme;

/// Reúne tudo relacionado a "tela": aparência, bloqueio de tela, papel de
/// parede, o menu e o dock do Sheliak (quando instalado) —
/// uma única entrada de navegação com abas internas, como o módulo Software.
#[derive(Clone)]
pub struct ScreenPage {
    pub root: gtk::Widget,
    pub screensaver: ScreensaverPage,
    pub wallpaper: WallpaperPage,
    pub menu: MenuPage,
    pub dock: DockPage,
}

impl ScreenPage {
    pub fn new() -> Self {
        let screensaver = ScreensaverPage::new();
        let wallpaper = WallpaperPage::new();
        let menu = MenuPage::new();
        let dock = DockPage::new();

        let appearance_tab = tab_button(&gettext("Tema"));
        let profile_tab = tab_button(&gettext("Perfil"));
        let wallpaper_tab = tab_button(&gettext("Papel de Parede"));
        let screensaver_tab = tab_button(&gettext("Proteção de Tela"));
        let menu_tab = tab_button(&gettext("Menu"));
        let dock_tab = tab_button(&gettext("Dock"));
        appearance_tab.set_active(true);
        profile_tab.set_group(Some(&appearance_tab));
        wallpaper_tab.set_group(Some(&appearance_tab));
        screensaver_tab.set_group(Some(&appearance_tab));
        menu_tab.set_group(Some(&appearance_tab));
        dock_tab.set_group(Some(&appearance_tab));
        // As abas permanecem visíveis para deixar clara a diferença entre os
        // perfis, mas só podem ser abertas quando o Sheliak está ativo.
        let sheliak_available = crate::dock::is_installed();
        let sheliak_enabled = sheliak_available && crate::dock::is_enabled();
        menu_tab.set_sensitive(sheliak_enabled);
        dock_tab.set_sensitive(sheliak_enabled);
        let (appearance, profile) = appearance_pages(&menu_tab, &dock_tab, sheliak_available);

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabs.add_css_class("module-tabs");
        tabs.append(&appearance_tab);
        tabs.append(&profile_tab);
        tabs.append(&wallpaper_tab);
        tabs.append(&screensaver_tab);
        tabs.append(&menu_tab);
        tabs.append(&dock_tab);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .build();
        stack.add_named(&appearance, Some("appearance"));
        stack.add_named(&profile, Some("profile"));
        stack.add_named(&wallpaper.root, Some("wallpaper"));
        stack.add_named(&screensaver.root, Some("screensaver"));
        stack.add_named(&menu.root, Some("menu"));
        stack.add_named(&dock.root, Some("dock"));
        stack.set_visible_child_name("appearance");

        let appearance_stack = stack.clone();
        appearance_tab.connect_clicked(move |button| {
            if button.is_active() {
                appearance_stack.set_visible_child_name("appearance");
            }
        });
        let profile_stack = stack.clone();
        profile_tab.connect_clicked(move |button| {
            if button.is_active() {
                profile_stack.set_visible_child_name("profile");
            }
        });
        let screensaver_stack = stack.clone();
        screensaver_tab.connect_clicked(move |button| {
            if button.is_active() {
                screensaver_stack.set_visible_child_name("screensaver");
            }
        });
        let wallpaper_stack = stack.clone();
        wallpaper_tab.connect_clicked(move |button| {
            if button.is_active() {
                wallpaper_stack.set_visible_child_name("wallpaper");
            }
        });
        let menu_stack = stack.clone();
        menu_tab.connect_clicked(move |button| {
            if button.is_active() {
                menu_stack.set_visible_child_name("menu");
            }
        });
        let dock_stack = stack.clone();
        dock_tab.connect_clicked(move |button| {
            if button.is_active() {
                dock_stack.set_visible_child_name("dock");
            }
        });

        let heading = gtk::Box::new(gtk::Orientation::Vertical, 4);
        heading.append(
            &gtk::Label::builder()
                .label(gettext("Personalização"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        heading.append(
            &gtk::Label::builder()
                .label(gettext("Aparência, bloqueio de tela e papel de parede"))
                .xalign(0.0)
                .css_classes(["dim-label"])
                .build(),
        );

        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.add_css_class("content-page");
        content.append(&heading);
        content.append(&tabs);
        content.append(&stack);

        Self {
            root: content.upcast(),
            screensaver,
            wallpaper,
            menu,
            dock,
        }
    }
}

impl Default for ScreenPage {
    fn default() -> Self {
        Self::new()
    }
}

fn tab_button(label: &str) -> gtk::ToggleButton {
    gtk::ToggleButton::builder()
        .label(label)
        .css_classes(["flat", "module-tab"])
        .build()
}

/// O tema escreve direto em `org.gnome.desktop.interface` (veja
/// `crate::appearance`): não é preferência do Vega, é a mesma
/// configuração do painel Aparência do GNOME — muda o Shell, o Nautilus e
/// qualquer app libadwaita em execução, não só a janela do Vega.
fn appearance_pages(
    menu_tab: &gtk::ToggleButton,
    dock_tab: &gtk::ToggleButton,
    sheliak_available: bool,
) -> (gtk::Widget, gtk::Widget) {
    let unavailable = !crate::appearance::schema_available();

    let theme_group = adw::PreferencesGroup::builder()
        .title(gettext("Tema"))
        .valign(gtk::Align::Start)
        .build();
    theme_group.set_margin_top(12);

    let cards = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    cards.set_homogeneous(true);
    cards.set_hexpand(true);
    cards.set_valign(gtk::Align::Start);

    // O GSetting pode estar em "default" (segue o sistema) mesmo sem esta UI
    // oferecer mais essa opção — nesse caso o card selecionado reflete a
    // aparência efetiva atual (resolvida pelo libadwaita), não força um valor.
    let resolved_dark = match crate::appearance::current_theme() {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => adw::StyleManager::default().is_dark(),
    };

    let light_card = theme_card(false, gettext("Claro"), None);
    let dark_card = theme_card(true, gettext("Escuro"), Some(&light_card));
    light_card.set_sensitive(!unavailable);
    dark_card.set_sensitive(!unavailable);
    light_card.set_active(!resolved_dark);
    dark_card.set_active(resolved_dark);

    light_card.connect_toggled(|button| {
        if button.is_active() {
            crate::appearance::apply_theme(Theme::Light);
            apply_enterprise_wallpaper();
            crate::appearance::apply_icon_theme();
        }
    });
    dark_card.connect_toggled(|button| {
        if button.is_active() {
            crate::appearance::apply_theme(Theme::Dark);
            apply_enterprise_wallpaper();
            crate::appearance::apply_icon_theme();
        }
    });

    cards.append(&light_card);
    cards.append(&dark_card);
    theme_group.add(&cards);

    let lyra_profile = profile_card(
        &gettext("Lyra"),
        &gettext("GNOME mais Dock e Menu do Lyra."),
        true,
        None,
    );
    let vanilla_profile = profile_card(
        &gettext("Gnome Vanila"),
        &gettext("Usa a experiência padrão do GNOME."),
        false,
        Some(&lyra_profile),
    );
    lyra_profile.set_sensitive(sheliak_available);
    lyra_profile.set_active(sheliak_available && crate::dock::is_enabled());
    vanilla_profile.set_active(!lyra_profile.is_active());

    let lyra_menu_tab = menu_tab.clone();
    let lyra_dock_tab = dock_tab.clone();
    lyra_profile.connect_toggled(move |button| {
        if button.is_active() && crate::dock::set_enabled(true).is_ok() {
            lyra_menu_tab.set_sensitive(true);
            lyra_dock_tab.set_sensitive(true);
        }
    });
    let vanilla_menu_tab = menu_tab.clone();
    let vanilla_dock_tab = dock_tab.clone();
    vanilla_profile.connect_toggled(move |button| {
        if button.is_active() && crate::dock::set_enabled(false).is_ok() {
            vanilla_menu_tab.set_sensitive(false);
            vanilla_dock_tab.set_sensitive(false);
        }
    });

    let profiles = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    profiles.set_homogeneous(true);
    profiles.set_valign(gtk::Align::Start);
    profiles.append(&lyra_profile);
    profiles.append(&vanilla_profile);

    let profile_group = adw::PreferencesGroup::builder()
        .title(gettext("Perfil da área de trabalho"))
        .valign(gtk::Align::Start)
        .build();
    profile_group.add(&profiles);

    let theme_content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    theme_content.set_valign(gtk::Align::Start);
    if unavailable {
        theme_content.append(
            &gtk::Label::builder()
                .label(gettext(
                    "Este sistema não tem os esquemas do GNOME para aparência; as opções abaixo ficam desativadas.",
                ))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
    }
    theme_content.append(&theme_group);

    let profile_content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    profile_content.set_valign(gtk::Align::Start);
    profile_content.append(&profile_group);

    let theme_page = gtk::ScrolledWindow::builder()
        .child(&theme_content)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build()
        .upcast();
    let profile_page = gtk::ScrolledWindow::builder()
        .child(&profile_content)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build()
        .upcast();
    (theme_page, profile_page)
}

fn profile_card(
    title: &str,
    description: &str,
    lyra: bool,
    group: Option<&gtk::ToggleButton>,
) -> gtk::ToggleButton {
    let title = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let description = gtk::Label::builder()
        .label(description)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&profile_preview(lyra));
    content.append(&title);
    content.append(&description);
    let button = gtk::ToggleButton::builder()
        .child(&content)
        .css_classes(["flat", "vega-profile-card"])
        .build();
    if let Some(group) = group {
        button.set_group(Some(group));
    }
    button
}

/// Ilustração compacta do desktop de cada perfil. É construída com widgets e
/// CSS (sem imagem externa): Lyra tem painel flutuante e dock lateral; GNOME
/// Vanilla tem painel colado ao topo e dash central inferior.
fn profile_preview(lyra: bool) -> gtk::Widget {
    let desktop = gtk::Box::new(gtk::Orientation::Vertical, 0);
    desktop.add_css_class("vega-profile-preview");

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&desktop));

    let panel = gtk::Box::new(gtk::Orientation::Horizontal, 3);
    panel.add_css_class("vega-profile-preview-panel");
    panel.set_halign(gtk::Align::Fill);
    panel.set_valign(gtk::Align::Start);
    for _ in 0..3 {
        let item = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        item.add_css_class("vega-profile-preview-item");
        panel.append(&item);
    }
    if lyra {
        panel.add_css_class("vega-profile-preview-panel-lyra");
        panel.set_margin_top(7);
        panel.set_margin_start(9);
        panel.set_margin_end(9);
    } else {
        panel.add_css_class("vega-profile-preview-panel-gnome");
    }
    overlay.add_overlay(&panel);

    let dock = gtk::Box::new(
        if lyra {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        },
        4,
    );
    dock.add_css_class("vega-profile-preview-dock");
    dock.add_css_class(if lyra {
        "vega-profile-preview-dock-lyra"
    } else {
        "vega-profile-preview-dock-gnome"
    });
    for _ in 0..4 {
        let icon = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        icon.add_css_class("vega-profile-preview-icon");
        dock.append(&icon);
    }
    if lyra {
        dock.set_halign(gtk::Align::Start);
        dock.set_valign(gtk::Align::Center);
        dock.set_margin_start(7);
    } else {
        dock.set_halign(gtk::Align::Center);
        dock.set_valign(gtk::Align::End);
        dock.set_margin_bottom(7);
    }
    overlay.add_overlay(&dock);
    overlay.upcast()
}

/// Ao trocar o card de tema, também força o papel de parede padrão do
/// Lyra OS (par `os.png`/`os-light.png`, entrada "Lyra OS" no XML de
/// gnome-background-properties) — o GNOME já troca sozinho entre eles depois
/// disso, via `picture-uri-dark`, então basta garantir que as duas URIs
/// estejam apontando pra esse par.
///
/// O nome mudou de "Lyra Enterprise" pra "Lyra OS" quando o Lyra-Theme foi
/// renomeado (Lyra-Theme@47d0ff4); a busca exata evita casar com as
/// variantes de humor adicionais ("Lyra OS — Nebula" etc.) que o Lyra-Theme
/// também registra.
fn apply_enterprise_wallpaper() {
    if let Some(entry) = crate::wallpaper::list_wallpapers()
        .into_iter()
        .find(|entry| entry.name.eq_ignore_ascii_case("Lyra OS"))
    {
        let _ = crate::wallpaper::apply(&entry);
    }
}

/// Card grande de seleção de tema: janela em miniatura (clara ou escura) com
/// o nome do tema embaixo, igual ao seletor de estilo das Configurações do
/// GNOME — bem mais reconhecível que um combo de texto.
fn theme_card(
    dark: bool,
    label_text: String,
    group: Option<&gtk::ToggleButton>,
) -> gtk::ToggleButton {
    let preview = theme_preview(dark);

    let label = gtk::Label::builder()
        .label(&label_text)
        .css_classes(["heading"])
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.append(&preview);
    content.append(&label);

    let button = gtk::ToggleButton::builder()
        .child(&content)
        .css_classes(["flat", "vega-theme-card"])
        .height_request(150)
        .hexpand(true)
        .halign(gtk::Align::Fill)
        .valign(gtk::Align::Start)
        .build();
    if let Some(group) = group {
        button.set_group(Some(group));
    }
    button
}

/// Miniatura de janela (barra de título com três pontos + duas linhas de
/// conteúdo) só para dar contexto visual ao card — não é uma janela real.
fn theme_preview(dark: bool) -> gtk::Widget {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    header.add_css_class("vega-window-preview-header");
    header.set_valign(gtk::Align::Start);
    for _ in 0..3 {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("vega-window-preview-dot");
        header.append(&dot);
    }

    let line_a = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    line_a.add_css_class("vega-window-preview-line");
    line_a.set_size_request(96, -1);
    line_a.set_halign(gtk::Align::Start);

    let line_b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    line_b.add_css_class("vega-window-preview-line");
    line_b.set_size_request(64, -1);
    line_b.set_halign(gtk::Align::Start);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.add_css_class("vega-window-preview-body");
    body.set_margin_top(12);
    body.set_margin_start(12);
    body.set_margin_end(12);
    body.set_vexpand(true);
    body.append(&line_a);
    body.append(&line_b);

    let window = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.add_css_class("vega-window-preview");
    window.add_css_class(if dark {
        "vega-window-preview-dark"
    } else {
        "vega-window-preview-light"
    });
    window.append(&header);
    window.append(&body);
    window.upcast()
}
