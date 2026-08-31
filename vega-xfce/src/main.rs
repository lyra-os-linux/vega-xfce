mod appearance;
mod application;
mod dock;
mod i18n;
mod model;
mod preferences;
mod screensaver;
mod ui;
mod wallpaper;

fn main() -> gtk::glib::ExitCode {
    configure_renderer();
    i18n::init();
    application::run()
}

fn configure_renderer() {
    if std::env::var_os("GSK_RENDERER").is_some() {
        return;
    }

    // This runs before GTK, GLib, zbus, or any worker thread is initialized.
    // Cairo avoids a Mesa llvmpipe crash seen while resizing/maximizing the
    // application in virtual machines, while still allowing an explicit
    // GSK_RENDERER override for diagnostics and accelerated environments.
    unsafe {
        std::env::set_var("GSK_RENDERER", "cairo");
    }
}
mod assistant;
