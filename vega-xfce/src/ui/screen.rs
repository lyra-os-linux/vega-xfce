use crate::appearance::Module;
use crate::i18n::gettext;
use adw::prelude::*;

use super::{DockPage, MenuPage, ScreensaverPage, WallpaperPage};

/// Central de personalização da edição XFCE. O papel de parede continua
/// integrado ao Vega; os cards abrem os configuradores nativos, que são a
/// fonte de verdade das demais preferências do desktop.
#[derive(Clone)]
pub struct ScreenPage {
    pub root: gtk::Widget,
    pub screensaver: ScreensaverPage,
    pub wallpaper: WallpaperPage,
    // Mantidos até a ligação antiga da aplicação ser removida por completo.
    // Os módulos GNOME/Sheliak não aparecem na interface XFCE.
    pub menu: MenuPage,
    pub dock: DockPage,
}

impl ScreenPage {
    pub fn new() -> Self {
        let screensaver = ScreensaverPage::new();
        let wallpaper = WallpaperPage::new();
        let menu = MenuPage::new();
        let dock = DockPage::new();

        let overview = native_page();
        let overview_tab = tab_button(&gettext("Visão geral"));
        let wallpaper_tab = tab_button(&gettext("Papel de parede"));
        overview_tab.set_active(true);
        wallpaper_tab.set_group(Some(&overview_tab));

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabs.add_css_class("module-tabs");
        tabs.append(&overview_tab);
        tabs.append(&wallpaper_tab);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .build();
        stack.add_named(&overview, Some("overview"));
        stack.add_named(&wallpaper.root, Some("wallpaper"));
        stack.set_visible_child_name("overview");
        connect_tab(&overview_tab, &stack, "overview");
        connect_tab(&wallpaper_tab, &stack, "wallpaper");

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
                .label(gettext("Ajuste a aparência e o comportamento do XFCE"))
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

fn connect_tab(button: &gtk::ToggleButton, stack: &gtk::Stack, page: &'static str) {
    let stack = stack.clone();
    button.connect_clicked(move |button| {
        if button.is_active() {
            stack.set_visible_child_name(page);
        }
    });
}

fn native_page() -> gtk::Widget {
    let grid = gtk::FlowBox::builder()
        .column_spacing(14)
        .row_spacing(14)
        .min_children_per_line(1)
        .max_children_per_line(2)
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .valign(gtk::Align::Start)
        .build();
    for (title, description, icon, module) in [
        (
            "Aparência",
            "Tema, cores, ícones, fontes e cursor",
            "preferences-desktop-theme-symbolic",
            Module::Appearance,
        ),
        (
            "Janelas",
            "Bordas, botões, foco e comportamento",
            "preferences-system-windows-symbolic",
            Module::Windows,
        ),
        (
            "Área de trabalho",
            "Papel de parede, menus e ícones do desktop",
            "preferences-desktop-wallpaper-symbolic",
            Module::Desktop,
        ),
        (
            "Painel",
            "Posição, tamanho e plugins do painel",
            "preferences-desktop-panel-symbolic",
            Module::Panel,
        ),
        (
            "Energia",
            "Suspensão, bateria, tela e economia de energia",
            "preferences-system-power-symbolic",
            Module::Power,
        ),
        (
            "Todas as configurações",
            "Abra o gerenciador de configurações do XFCE",
            "preferences-system-symbolic",
            Module::Settings,
        ),
    ] {
        grid.insert(
            &native_card(&gettext(title), &gettext(description), icon, module),
            -1,
        );
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.append(&grid);
    gtk::ScrolledWindow::builder()
        .child(&content)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build()
        .upcast()
}

fn native_card(title: &str, description: &str, icon: &str, module: Module) -> gtk::Button {
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(36);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
    labels.set_hexpand(true);
    labels.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["heading"])
            .build(),
    );
    labels.append(
        &gtk::Label::builder()
            .label(description)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build(),
    );
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.set_margin_top(16);
    row.set_margin_bottom(16);
    row.set_margin_start(16);
    row.set_margin_end(16);
    row.append(&image);
    row.append(&labels);
    row.append(&gtk::Image::from_icon_name("go-next-symbolic"));
    let button = gtk::Button::builder()
        .child(&row)
        .css_classes(["flat", "card"])
        .height_request(112)
        .hexpand(true)
        .build();
    button.connect_clicked(move |_| {
        if let Err(error) = crate::appearance::open_module(module) {
            eprintln!("não foi possível abrir o módulo nativo do XFCE: {error}");
        }
    });
    button
}
