use crate::appearance::Module;
use crate::i18n::gettext;
use adw::prelude::*;

use super::{DockPage, MenuPage, ScreensaverPage, WallpaperPage};

/// Central de personalização da edição XFCE. Como no Vega Qt, os cards são
/// atalhos para os configuradores nativos, que permanecem como fonte de
/// verdade das preferências do desktop.
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
        content.set_hexpand(true);
        content.set_vexpand(true);
        content.append(&heading);
        content.append(&native_page());
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

fn native_page() -> gtk::Widget {
    let grid = gtk::FlowBox::builder()
        .column_spacing(14)
        .row_spacing(14)
        .min_children_per_line(1)
        .max_children_per_line(2)
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .hexpand(true)
        .halign(gtk::Align::Fill)
        .valign(gtk::Align::Start)
        .build();
    for (title, description, icon, module) in [
        (
            "Aparência",
            "Tema, cores, ícones, fontes e cursor",
            "preferences-desktop-appearance-symbolic",
            Module::Appearance,
        ),
        (
            "Janelas",
            "Bordas, botões, foco e comportamento",
            "window-new-symbolic",
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
            "view-grid-symbolic",
            Module::Panel,
        ),
        (
            "Bloqueio de tela",
            "Proteção, bloqueio automático e tempo de inatividade",
            "system-lock-screen-symbolic",
            Module::Screensaver,
        ),
        (
            "Energia",
            "Suspensão, bateria, tela e economia de energia",
            "battery-symbolic",
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
    content.set_hexpand(true);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.append(&grid);
    gtk::ScrolledWindow::builder()
        .child(&content)
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_height(false)
        .build()
        .upcast()
}

fn native_card(title: &str, description: &str, icon: &str, module: Module) -> gtk::Button {
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(36);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
    labels.set_hexpand(true);
    labels.set_halign(gtk::Align::Fill);
    labels.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .hexpand(true)
            .width_chars(18)
            .css_classes(["heading"])
            .build(),
    );
    labels.append(
        &gtk::Label::builder()
            .label(description)
            .xalign(0.0)
            .hexpand(true)
            .width_chars(24)
            .max_width_chars(42)
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
        .width_request(330)
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
