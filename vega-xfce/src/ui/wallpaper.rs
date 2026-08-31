use std::{cell::RefCell, path::Path, rc::Rc};

use crate::i18n::gettext;
use adw::prelude::*;

use crate::wallpaper::{ThumbnailData, WallpaperEntry, thumbnail_to_pixbuf};

type ApplyHandler = Rc<dyn Fn(WallpaperEntry)>;

#[derive(Clone)]
pub struct WallpaperPage {
    #[allow(dead_code)]
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub add: gtk::Button,
    grid: gtk::FlowBox,
    apply_handlers: Rc<RefCell<Vec<ApplyHandler>>>,
}

impl WallpaperPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando papéis de parede…"))
            .xalign(0.0)
            .hexpand(true)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let add_content = adw::ButtonContent::builder()
            .icon_name("list-add-symbolic")
            .label(gettext("Adicionar wallpaper…"))
            .build();
        let add = gtk::Button::builder()
            .child(&add_content)
            .css_classes(["suggested-action"])
            .tooltip_text(gettext("Adicionar uma imagem do seu computador"))
            .build();
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.append(&status);
        header.append(&add);
        let grid = gtk::FlowBox::builder()
            .column_spacing(12)
            .row_spacing(12)
            .min_children_per_line(2)
            .max_children_per_line(5)
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.append(&header);
        content.append(&grid);

        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();

        Self {
            root,
            status,
            add,
            grid,
            apply_handlers: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn connect_apply(&self, handler: impl Fn(WallpaperEntry) + 'static) {
        self.apply_handlers.borrow_mut().push(Rc::new(handler));
    }

    pub fn show(
        &self,
        wallpapers: &[WallpaperEntry],
        thumbnails: &[Option<ThumbnailData>],
        current: Option<&Path>,
    ) {
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        if wallpapers.is_empty() {
            self.status
                .set_label(&gettext("Nenhum papel de parede encontrado neste sistema."));
            return;
        }
        self.status.set_label(
            &gettext("{count} papéis de parede encontrados")
                .replace("{count}", &wallpapers.len().to_string()),
        );
        for (wallpaper, thumbnail) in wallpapers.iter().zip(thumbnails) {
            let is_current = current == Some(wallpaper.light_path.as_path());
            let card = build_card(wallpaper, thumbnail.as_ref(), is_current);
            let handlers = self.apply_handlers.clone();
            let entry = wallpaper.clone();
            card.connect_clicked(move |_| {
                for handler in handlers.borrow().iter() {
                    handler(entry.clone());
                }
            });
            self.grid.insert(&card, -1);
        }
    }
}

impl Default for WallpaperPage {
    fn default() -> Self {
        Self::new()
    }
}

fn build_card(
    entry: &WallpaperEntry,
    thumbnail: Option<&ThumbnailData>,
    is_current: bool,
) -> gtk::Button {
    // Os bytes já vêm decodificados (em escala reduzida) de uma thread de
    // fundo — decodificar aqui na thread principal, um wallpaper de cada
    // vez, é o que travava a UI antes (arquivos 4K, ~30 de uma vez).
    // Remontar o Pixbuf a partir dos bytes é rápido, sem decode de imagem.
    let thumbnail_widget: gtk::Widget = match thumbnail {
        Some(data) => gtk::Image::from_pixbuf(Some(&thumbnail_to_pixbuf(data))).upcast(),
        // Formatos que o gdk-pixbuf não sabe decodificar neste sistema (ou
        // descritores de slideshow em XML, como os "Time of Day" do GNOME)
        // caem aqui — o papel de parede continua aplicável, só não tem preview.
        None => gtk::Image::builder()
            .icon_name("image-x-generic-symbolic")
            .pixel_size(48)
            .build()
            .upcast(),
    };
    thumbnail_widget.set_size_request(160, 96);

    let frame = gtk::Frame::new(None);
    frame.set_child(Some(&thumbnail_widget));
    frame.add_css_class("wallpaper-thumbnail");

    let label = gtk::Label::builder()
        .label(&entry.name)
        .xalign(0.0)
        .wrap(true)
        .lines(2)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["caption"])
        .build();

    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    body.append(&frame);
    body.append(&label);

    let button = gtk::Button::builder()
        .child(&body)
        .css_classes(["flat", "wallpaper-card"])
        .build();
    if is_current {
        button.add_css_class("wallpaper-card-active");
    }
    button
}
