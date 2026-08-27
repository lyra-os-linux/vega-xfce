use std::{
    collections::BTreeMap,
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::Command,
};

use crate::i18n::gettext;
use gtk::{gio, gio::prelude::*, glib};

const SCHEMA_ID: &str = "org.gnome.desktop.background";
const XFCE_CHANNEL: &str = "xfce4-desktop";
const CATALOG_DIRS: &[&str] = &[
    "/usr/share/gnome-background-properties",
    "/usr/local/share/gnome-background-properties",
];
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "svg", "webp", "jxl", "bmp", "avif"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperEntry {
    pub name: String,
    pub light_path: PathBuf,
    pub dark_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperError(String);

impl std::fmt::Display for WallpaperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for WallpaperError {}

/// O wallpaper pertence à sessão do usuário e não passa pelo vegad. No XFCE,
/// o xfdesktop publica essa configuração no canal `xfce4-desktop`; o backend
/// GNOME é mantido como fallback para desenvolvimento e sessões compatíveis.
pub fn schema_available() -> bool {
    if is_xfce_session() {
        return xfconf_properties().is_ok();
    }
    gio::SettingsSchemaSource::default()
        .is_some_and(|source| source.lookup(SCHEMA_ID, true).is_some())
}

pub fn list_wallpapers() -> Vec<WallpaperEntry> {
    let mut catalog_entries = Vec::new();
    for dir in CATALOG_DIRS {
        catalog_entries.extend(parse_catalog_dir(Path::new(dir)));
    }
    if let Some(home) = glib::home_dir().to_str().map(PathBuf::from) {
        catalog_entries.extend(parse_catalog_dir(
            &home.join(".local/share/gnome-background-properties"),
        ));
    }

    let mut by_light_path = BTreeMap::<PathBuf, WallpaperEntry>::new();
    // Variantes escuras já aparecem penduradas numa entrada de catálogo
    // (dark_path); sem esse controle, a varredura de fallback abaixo lista
    // o mesmo arquivo de novo como se fosse um papel de parede à parte.
    let mut known_dark_paths = std::collections::BTreeSet::<PathBuf>::new();
    for entry in catalog_entries {
        if let Some(dark) = &entry.dark_path {
            known_dark_paths.insert(dark.clone());
        }
        by_light_path
            .entry(entry.light_path.clone())
            .or_insert(entry);
    }

    // Papéis instalados pelo sistema são somente os registrados nos catálogos
    // GNOME acima. Não varremos /usr/share/backgrounds: pacotes podem manter
    // ali formatos auxiliares (por exemplo PNG + JXL do mesmo papel), imagens
    // internas e variantes claro/escuro que não são escolhas independentes.
    // A única pasta sem catálogo aceita é a gerenciada pelo próprio Vega para
    // os papéis importados pelo usuário.
    for path in scan_image_files(&user_wallpaper_dir()) {
        if known_dark_paths.contains(&path) || by_light_path.contains_key(&path) {
            continue;
        }
        by_light_path
            .entry(path.clone())
            .or_insert_with(|| WallpaperEntry {
                name: display_name_from_path(&path),
                light_path: path,
                dark_path: None,
            });
    }

    let mut wallpapers = by_light_path.into_values().collect::<Vec<_>>();
    wallpapers.sort_by_key(|entry| entry.name.to_lowercase());
    wallpapers
}

fn user_wallpaper_dir() -> PathBuf {
    glib::user_data_dir().join("backgrounds")
}

/// Copia uma imagem escolhida pelo usuário para uma pasta persistente e
/// gerenciada pela própria sessão. Isso evita que o papel de parede pare de
/// funcionar quando a origem era um pendrive, um download temporário ou um
/// arquivo disponibilizado pelo portal de documentos.
pub fn import(source: &Path) -> Result<WallpaperEntry, WallpaperError> {
    import_to_dir(source, &user_wallpaper_dir())
}

fn import_to_dir(source: &Path, destination_dir: &Path) -> Result<WallpaperEntry, WallpaperError> {
    if !source.is_file() {
        return Err(WallpaperError(gettext(
            "Selecione um arquivo de imagem local.",
        )));
    }

    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|extension| IMAGE_EXTENSIONS.contains(&extension.as_str()))
        .ok_or_else(|| WallpaperError(gettext("Formato de imagem não compatível.")))?;
    if gtk::gdk_pixbuf::Pixbuf::file_info(source).is_none() {
        return Err(WallpaperError(gettext(
            "O arquivo selecionado não é uma imagem válida.",
        )));
    }

    fs::create_dir_all(destination_dir).map_err(|error| {
        WallpaperError(
            gettext("Não foi possível criar a pasta de wallpapers: {error}")
                .replace("{error}", &error.to_string()),
        )
    })?;

    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("wallpaper");
    let mut suffix = 1_u32;
    loop {
        let filename = if suffix == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{suffix}.{extension}")
        };
        let destination = destination_dir.join(filename);
        if destination.is_file() {
            if files_have_same_contents(source, &destination).unwrap_or(false) {
                return Ok(entry_from_path(destination));
            }
            suffix += 1;
            continue;
        }

        fs::copy(source, &destination).map_err(|error| {
            WallpaperError(
                gettext("Não foi possível adicionar o wallpaper: {error}")
                    .replace("{error}", &error.to_string()),
            )
        })?;
        return Ok(entry_from_path(destination));
    }
}

fn entry_from_path(path: PathBuf) -> WallpaperEntry {
    WallpaperEntry {
        name: display_name_from_path(&path),
        light_path: path,
        dark_path: None,
    }
}

fn files_have_same_contents(first: &Path, second: &Path) -> std::io::Result<bool> {
    if fs::metadata(first)?.len() != fs::metadata(second)?.len() {
        return Ok(false);
    }

    let mut first = BufReader::new(fs::File::open(first)?);
    let mut second = BufReader::new(fs::File::open(second)?);
    let mut first_buffer = [0_u8; 8192];
    let mut second_buffer = [0_u8; 8192];
    loop {
        let first_read = first.read(&mut first_buffer)?;
        let second_read = second.read(&mut second_buffer)?;
        if first_read != second_read || first_buffer[..first_read] != second_buffer[..second_read] {
            return Ok(false);
        }
        if first_read == 0 {
            return Ok(true);
        }
    }
}

const THUMBNAIL_WIDTH: i32 = 160;
const THUMBNAIL_HEIGHT: i32 = 96;

/// Pixels já decodificados de uma miniatura, em formato simples (`Vec<u8>` +
/// metadados) para poder atravessar uma thread — `gdk_pixbuf::Pixbuf` não é
/// `Send` (GObject não thread-safe nos bindings), então não dá pra carregar o
/// Pixbuf em si numa `gio::spawn_blocking` e devolver pra thread principal.
pub struct ThumbnailData {
    bytes: Vec<u8>,
    colorspace: gtk::gdk_pixbuf::Colorspace,
    has_alpha: bool,
    bits_per_sample: i32,
    width: i32,
    height: i32,
    rowstride: i32,
}

/// Decodifica as miniaturas já em escala reduzida (não na resolução original
/// do arquivo — muitos desses papéis de parede são 4K) e já cortadas pelo
/// centro no formato do card ("cover", sem distorcer a imagem). Chamada
/// pensada para rodar numa thread de fundo (`gio::spawn_blocking`):
/// decodificar ~30 imagens grandes de uma vez na thread principal trava a UI
/// tempo suficiente pro compositor achar que o app não está respondendo.
pub fn load_thumbnails(
    entries: &[WallpaperEntry],
    prefer_dark: bool,
) -> Vec<Option<ThumbnailData>> {
    entries
        .iter()
        .map(|entry| load_cover_thumbnail(thumbnail_path(entry, prefer_dark)))
        .collect()
}

fn thumbnail_path(entry: &WallpaperEntry, prefer_dark: bool) -> &Path {
    if prefer_dark {
        entry.dark_path.as_deref().unwrap_or(&entry.light_path)
    } else {
        &entry.light_path
    }
}

fn load_cover_thumbnail(path: &Path) -> Option<ThumbnailData> {
    // Decodifica preenchendo a largura alvo (altura calculada preservando a
    // proporção) e corta o excesso pelo centro. Papéis de parede quase
    // sempre são widescreen, mas se a altura ficar menor que o alvo (imagem
    // mais "quadrada" ou vertical), decodifica pela altura em vez disso.
    let scaled = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, THUMBNAIL_WIDTH, -1, true)
        .ok()
        .filter(|pixbuf| pixbuf.height() >= THUMBNAIL_HEIGHT)
        .or_else(|| {
            gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, -1, THUMBNAIL_HEIGHT, true).ok()
        })?;
    Some(crop_center(&scaled, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT))
}

fn crop_center(
    pixbuf: &gtk::gdk_pixbuf::Pixbuf,
    target_width: i32,
    target_height: i32,
) -> ThumbnailData {
    let src_width = pixbuf.width();
    let src_height = pixbuf.height();
    let channels = if pixbuf.has_alpha() { 4 } else { 3 };
    let rowstride = pixbuf.rowstride();
    let target_width = target_width.min(src_width);
    let target_height = target_height.min(src_height);
    let x_offset = (src_width - target_width) / 2;
    let y_offset = (src_height - target_height) / 2;

    // `pixel_bytes()` só retorna Some quando o Pixbuf já é respaldado por um
    // GBytes imutável. Pixbufs carregados de arquivo normalmente mantêm um
    // buffer mutável e faziam a função devolver None, produzindo previews com
    // metadados válidos mas nenhum pixel. `read_pixel_bytes()` sempre fornece
    // uma visão imutável (copiando quando necessário).
    let pixels = pixbuf.read_pixel_bytes();
    let mut bytes = Vec::with_capacity((target_width * target_height * channels) as usize);
    for row in 0..target_height {
        let start = ((y_offset + row) * rowstride + x_offset * channels) as usize;
        let end = start + (target_width * channels) as usize;
        if let Some(slice) = pixels.get(start..end) {
            bytes.extend_from_slice(slice);
        }
    }

    ThumbnailData {
        bytes,
        colorspace: pixbuf.colorspace(),
        has_alpha: pixbuf.has_alpha(),
        bits_per_sample: pixbuf.bits_per_sample(),
        width: target_width,
        height: target_height,
        rowstride: target_width * channels,
    }
}

/// Remonta o Pixbuf a partir dos bytes já decodificados — operação rápida
/// (só empacota o buffer existente), segura de rodar na thread principal.
pub fn thumbnail_to_pixbuf(data: &ThumbnailData) -> gtk::gdk_pixbuf::Pixbuf {
    gtk::gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from(&data.bytes),
        data.colorspace,
        data.has_alpha,
        data.bits_per_sample,
        data.width,
        data.height,
        data.rowstride,
    )
}

pub fn current_light_path() -> Option<PathBuf> {
    if is_xfce_session() {
        return xfce_image_properties()
            .ok()?
            .into_iter()
            .find_map(|property| xfconf_get(&property));
    }
    if !schema_available() {
        return None;
    }
    let settings = gio::Settings::new(SCHEMA_ID);
    let uri = settings.string("picture-uri");
    gio::File::for_uri(&uri).path()
}

pub fn apply(entry: &WallpaperEntry) -> Result<(), WallpaperError> {
    if is_xfce_session() {
        return apply_xfce(&entry.light_path);
    }
    if !schema_available() {
        return Err(WallpaperError(gettext(
            "O esquema org.gnome.desktop.background não está disponível neste sistema.",
        )));
    }
    let settings = gio::Settings::new(SCHEMA_ID);
    let light_uri = gio::File::for_path(&entry.light_path).uri();
    let dark_uri = entry
        .dark_path
        .as_deref()
        .map(|path| gio::File::for_path(path).uri())
        .unwrap_or_else(|| light_uri.clone());
    settings
        .set_string("picture-uri", &light_uri)
        .map_err(|_| WallpaperError(gettext("Não foi possível aplicar o papel de parede.")))?;
    let _ = settings.set_string("picture-uri-dark", &dark_uri);
    let _ = settings.set_string("picture-options", "zoom");
    Ok(())
}

fn is_xfce_session() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .is_some_and(|desktop| {
            desktop
                .split(':')
                .any(|part| part.eq_ignore_ascii_case("xfce"))
        })
}

fn xfconf_properties() -> Result<Vec<String>, WallpaperError> {
    let output = Command::new("xfconf-query")
        .args(["-c", XFCE_CHANNEL, "-l"])
        .output()
        .map_err(|error| WallpaperError(format!("xfconf-query: {error}")))?;
    if !output.status.success() {
        return Err(WallpaperError(gettext(
            "Não foi possível acessar as configurações do desktop XFCE.",
        )));
    }
    Ok(parse_xfconf_properties(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn parse_xfconf_properties(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|property| {
            property.starts_with("/backdrop/")
                && (property.ends_with("/last-image") || property.ends_with("/image-path"))
        })
        .map(str::to_owned)
        .collect()
}

fn xfce_image_properties() -> Result<Vec<String>, WallpaperError> {
    let properties = xfconf_properties()?
        .into_iter()
        .filter(|property| property.ends_with("/last-image"))
        .collect::<Vec<_>>();
    if properties.is_empty() {
        // XFCE antigo usava `image-path` em vez de `last-image`.
        return Ok(xfconf_properties()?
            .into_iter()
            .filter(|property| property.ends_with("/image-path"))
            .collect());
    }
    Ok(properties)
}

fn xfconf_get(property: &str) -> Option<PathBuf> {
    let output = Command::new("xfconf-query")
        .args(["-c", XFCE_CHANNEL, "-p", property])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
        .filter(|path| !path.as_os_str().is_empty())
}

fn apply_xfce(path: &Path) -> Result<(), WallpaperError> {
    let properties = xfce_image_properties()?;
    if properties.is_empty() {
        return Err(WallpaperError(gettext(
            "O XFCE ainda não possui uma configuração de wallpaper para este monitor.",
        )));
    }
    for property in properties {
        let status = Command::new("xfconf-query")
            .args(["-c", XFCE_CHANNEL, "-p", &property, "-s"])
            .arg(path)
            .status()
            .map_err(|error| WallpaperError(format!("xfconf-query: {error}")))?;
        if !status.success() {
            return Err(WallpaperError(gettext(
                "Não foi possível aplicar o papel de parede no XFCE.",
            )));
        }
    }
    Ok(())
}

fn parse_catalog_dir(dir: &Path) -> Vec<WallpaperEntry> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for file in read_dir.flatten() {
        let path = file.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("xml") {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        entries.extend(parse_catalog_xml(&contents));
    }
    entries
}

fn parse_catalog_xml(contents: &str) -> Vec<WallpaperEntry> {
    // Os catálogos do GNOME sempre declaram
    // <!DOCTYPE wallpapers SYSTEM "gnome-wp-list.dtd">, e roxmltree rejeita
    // qualquer DTD por padrão — allow_dtd é necessário mesmo sem resolver o
    // arquivo .dtd externo (não há entidades para expandir).
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let Ok(document) = roxmltree::Document::parse_with_options(contents, options) else {
        return Vec::new();
    };
    document
        .descendants()
        .filter(|node| node.has_tag_name("wallpaper"))
        .filter(|node| {
            node.attribute("deleted")
                .is_none_or(|value| value != "true")
        })
        .filter_map(|node| {
            let name = child_text(node, "name")?;
            let light_path = PathBuf::from(child_text(node, "filename")?);
            if !light_path.is_file() {
                return None;
            }
            let dark_path = child_text(node, "filename-dark")
                .map(PathBuf::from)
                .filter(|path| path.is_file());
            Some(WallpaperEntry {
                name,
                light_path,
                dark_path,
            })
        })
        .collect()
}

fn child_text(node: roxmltree::Node, tag: &str) -> Option<String> {
    node.children()
        .find(|child| child.has_tag_name(tag))
        .and_then(|child| child.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn scan_image_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(read_dir) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let is_image = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.as_str()));
            if is_image {
                files.push(path);
            }
        }
    }
    files
}

fn display_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace(['-', '_'], " "))
        .map(|stem| {
            stem.split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| gettext("Papel de parede"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_current_and_legacy_xfce_wallpaper_properties() {
        let output = r#"
/backdrop/screen0/monitorDP-1/workspace0/last-image
/backdrop/screen0/monitorDP-1/workspace0/image-style
/backdrop/screen0/monitor0/image-path
/desktop-icons/style
"#;
        assert_eq!(
            parse_xfconf_properties(output),
            vec![
                "/backdrop/screen0/monitorDP-1/workspace0/last-image",
                "/backdrop/screen0/monitor0/image-path",
            ]
        );
    }

    #[test]
    fn parses_a_wallpaper_catalog_entry_with_existing_files() {
        let dir = std::env::temp_dir().join(format!("vega-wallpaper-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let light = dir.join("light.png");
        let dark = dir.join("dark.png");
        std::fs::write(&light, b"fake").unwrap();
        std::fs::write(&dark, b"fake").unwrap();

        let xml = format!(
            r#"<?xml version="1.0"?>
<wallpapers>
  <wallpaper deleted="false">
    <name>Amber</name>
    <filename>{}</filename>
    <filename-dark>{}</filename-dark>
  </wallpaper>
</wallpapers>"#,
            light.display(),
            dark.display()
        );
        let entries = parse_catalog_xml(&xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Amber");
        assert_eq!(entries[0].light_path, light);
        assert_eq!(entries[0].dark_path.as_deref(), Some(dark.as_path()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn deleted_entries_are_skipped() {
        let xml = r#"<?xml version="1.0"?>
<wallpapers>
  <wallpaper deleted="true">
    <name>Old</name>
    <filename>/nonexistent.png</filename>
  </wallpaper>
</wallpapers>"#;
        assert!(parse_catalog_xml(xml).is_empty());
    }

    #[test]
    fn display_name_from_path_title_cases_the_stem() {
        assert_eq!(
            display_name_from_path(Path::new("/tmp/futurecity_dark.webp")),
            "Futurecity Dark"
        );
    }

    #[test]
    fn thumbnail_follows_the_active_color_scheme_with_light_fallback() {
        let entry = WallpaperEntry {
            name: "Lyra".into(),
            light_path: PathBuf::from("/wallpapers/light.png"),
            dark_path: Some(PathBuf::from("/wallpapers/dark.png")),
        };
        assert_eq!(
            thumbnail_path(&entry, false),
            Path::new("/wallpapers/light.png")
        );
        assert_eq!(
            thumbnail_path(&entry, true),
            Path::new("/wallpapers/dark.png")
        );

        let light_only = WallpaperEntry {
            dark_path: None,
            ..entry
        };
        assert_eq!(
            thumbnail_path(&light_only, true),
            Path::new("/wallpapers/light.png")
        );
    }

    #[test]
    fn imports_a_valid_image_without_duplicating_it() {
        let root =
            std::env::temp_dir().join(format!("vega-wallpaper-import-test-{}", std::process::id()));
        let source_dir = root.join("source");
        let destination_dir = root.join("destination");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("minha-paisagem.png");
        let pixbuf =
            gtk::gdk_pixbuf::Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 2, 2).unwrap();
        pixbuf.fill(0x3584_e4ff);
        pixbuf.savev(&source, "png", &[]).unwrap();

        let imported = import_to_dir(&source, &destination_dir).unwrap();
        assert_eq!(imported.name, "Minha Paisagem");
        assert_eq!(
            imported.light_path,
            destination_dir.join("minha-paisagem.png")
        );
        assert!(imported.light_path.is_file());

        let imported_again = import_to_dir(&source, &destination_dir).unwrap();
        assert_eq!(imported_again.light_path, imported.light_path);
        assert!(!destination_dir.join("minha-paisagem-2.png").exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_a_file_that_is_not_an_image() {
        let root = std::env::temp_dir().join(format!(
            "vega-wallpaper-invalid-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("not-an-image.png");
        std::fs::write(&source, b"not really a PNG").unwrap();

        let result = import_to_dir(&source, &root.join("destination"));
        assert!(result.is_err());
        assert!(!root.join("destination").exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn loaded_thumbnail_contains_all_cropped_pixels() {
        let root = std::env::temp_dir().join(format!(
            "vega-wallpaper-thumbnail-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("wallpaper.png");
        let pixbuf =
            gtk::gdk_pixbuf::Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 320, 192)
                .unwrap();
        pixbuf.fill(0x3584_e4ff);
        pixbuf.savev(&source, "png", &[]).unwrap();

        let thumbnail = load_cover_thumbnail(&source).unwrap();
        assert_eq!(thumbnail.width, THUMBNAIL_WIDTH);
        assert_eq!(thumbnail.height, THUMBNAIL_HEIGHT);
        assert_eq!(
            thumbnail.bytes.len(),
            (thumbnail.rowstride * thumbnail.height) as usize
        );

        std::fs::remove_dir_all(&root).unwrap();
    }
}
