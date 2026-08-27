use gettextrs::{LocaleCategory, TextDomain};
use gtk::gio;
use gtk::gio::prelude::DBusProxyExt;
use gtk::glib;
use gtk::glib::variant::ToVariant;

const DOMAIN: &str = "vega-gtk";

/// Initializes gettext from the session's native message locale. Unsupported
/// locales are mapped to en-US before binding, making English the deterministic
/// fallback while keeping locale changes effective on the next launch.
pub fn init() {
    let locale = session_locale();
    init_locale(locale);
}

fn init_locale(locale: &str) {
    // SAFETY: called once on the GTK main thread, before worker threads and
    // before any translated widget is created.
    unsafe { std::env::set_var("LANGUAGE", locale) };
    // Além dos caminhos padrão do sistema (/usr/share/locale, usado pelo
    // pacote instalado), procura também os .mo que o build.rs acabou de
    // gerar em `po/`, pra `cargo run` local funcionar sem instalar nada.
    let local_path = concat!(env!("CARGO_MANIFEST_DIR"), "/po");
    let result = TextDomain::new(DOMAIN)
        .prepend(local_path)
        .locale(locale)
        .locale_category(LocaleCategory::LcMessages)
        .init();
    if let Err(error) = result {
        eprintln!("i18n: falling back to source strings after catalog error: {error}");
    }
}

fn session_locale() -> &'static str {
    let gnome = gnome_language();
    let environment = ["LC_ALL", "LC_MESSAGES", "LANG"].map(std::env::var);
    resolve_locale(
        gnome.as_deref(),
        environment.iter().filter_map(|value| value.as_deref().ok()),
    )
}

/// GNOME stores the language selected for the logged-in user in AccountsService.
/// Reading it over D-Bus avoids inheriting a stale process environment when the
/// user changes the language in GNOME Settings. The new language takes effect on
/// Vega's next launch, just like other GNOME applications.
fn gnome_language() -> Option<String> {
    let accounts = gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::NONE,
        None,
        "org.freedesktop.Accounts",
        "/org/freedesktop/Accounts",
        "org.freedesktop.Accounts",
        gio::Cancellable::NONE,
    )
    .ok()?;
    let username = glib::user_name().to_string_lossy().into_owned();
    let reply = accounts
        .call_sync(
            "FindUserByName",
            Some(&(username.as_str(),).to_variant()),
            gio::DBusCallFlags::NONE,
            1_000,
            gio::Cancellable::NONE,
        )
        .ok()?;
    let (path,) = reply.get::<(glib::variant::ObjectPath,)>()?;
    let user = gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::NONE,
        None,
        "org.freedesktop.Accounts",
        &path,
        "org.freedesktop.Accounts.User",
        gio::Cancellable::NONE,
    )
    .ok()?;
    user.cached_property("Language")?
        .get::<String>()
        .filter(|value| !value.trim().is_empty())
}

fn resolve_locale<'a>(
    gnome: Option<&'a str>,
    environment: impl IntoIterator<Item = &'a str>,
) -> &'static str {
    gnome
        .into_iter()
        .chain(environment)
        .into_iter()
        .find(|value| !value.trim().is_empty() && !is_portable_locale(value))
        .map_or("en_US", normalize_locale)
}

/// `C` and `POSIX` describe a portable process environment, not the language
/// selected by the user. Continue to the next locale variable when launchers
/// set either value globally while `LANG` still carries the desktop language.
fn is_portable_locale(value: &str) -> bool {
    let base = value.trim().split('@').next().unwrap_or("");
    let base = base.split('.').next().unwrap_or("");
    base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX")
}

fn normalize_locale(value: &str) -> &'static str {
    let base = value.trim().split('@').next().unwrap_or("");
    let base = base.split('.').next().unwrap_or("").replace('_', "-");
    match base.to_ascii_lowercase().as_str() {
        "en-us" => "en_US",
        "pt-br" => "pt_BR",
        "es-es" => "es_ES",
        _ => "en_US",
    }
}

pub fn gettext(message: &str) -> String {
    gettextrs::gettext(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_normalization_and_fallback() {
        for (input, expected) in [
            ("en_US.UTF-8", "en_US"),
            ("pt_BR.UTF-8", "pt_BR"),
            ("es_ES.UTF-8@custom", "es_ES"),
            ("zh_CN.UTF-8", "en_US"),
            ("fr_FR.UTF-8", "en_US"),
            ("../../pt_BR", "en_US"),
        ] {
            assert_eq!(normalize_locale(input), expected);
        }
    }

    #[test]
    fn gnome_language_takes_precedence_over_environment() {
        assert_eq!(
            resolve_locale(Some("es_ES.UTF-8"), ["pt_BR.UTF-8"]),
            "es_ES"
        );
        assert_eq!(resolve_locale(None, ["zh_CN.UTF-8"]), "en_US");
        assert_eq!(resolve_locale(None, []), "en_US");
    }

    #[test]
    fn portable_locale_does_not_hide_desktop_language() {
        assert_eq!(
            resolve_locale(None, ["C.UTF-8", "C.UTF-8", "pt_BR.UTF-8"]),
            "pt_BR"
        );
        assert_eq!(
            resolve_locale(Some("C.UTF-8"), ["POSIX", "es_ES.UTF-8"]),
            "es_ES"
        );
        assert_eq!(resolve_locale(None, ["C.UTF-8", "POSIX"]), "en_US");
    }

    /// Runs in a dedicated process because gettext's locale and active domain
    /// are process-global state and Rust executes unit tests concurrently.
    #[test]
    fn translation_subprocess() {
        let Ok(locale) = std::env::var("VEGA_GTK_TEST_LOCALE") else {
            return;
        };
        let expected = std::env::var("VEGA_GTK_TEST_EXPECTED").unwrap();
        init_locale(&locale);
        assert_eq!(gettext("Painel"), expected);
    }

    #[test]
    fn loads_each_catalog_even_when_parent_locale_is_portable() {
        let executable = std::env::current_exe().unwrap();
        for (locale, expected) in [
            ("en_US", "Dashboard"),
            ("pt_BR", "Painel"),
            ("es_ES", "Panel de control"),
        ] {
            let status = std::process::Command::new(&executable)
                .arg("--exact")
                .arg("i18n::tests::translation_subprocess")
                .arg("--nocapture")
                .env("LC_ALL", "C.UTF-8")
                .env("LC_MESSAGES", "C.UTF-8")
                .env("LANG", "C.UTF-8")
                .env("VEGA_GTK_TEST_LOCALE", locale)
                .env("VEGA_GTK_TEST_EXPECTED", expected)
                .status()
                .unwrap();
            assert!(status.success(), "failed to load {locale} catalog");
        }
    }
}
