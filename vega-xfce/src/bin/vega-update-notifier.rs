use gettextrs::gettext;
use gtk::gio::prelude::*;
use gtk::{gio, glib};
use lyra_vega_dbus::{SoftwareClient, SoftwareEvent, VegaDbus};
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

const APPLICATION_ID: &str = "org.lyraos.Vega.UpdateNotifier";

fn main() -> glib::ExitCode {
    let _ = gettextrs::TextDomain::new("vega-gtk").init();
    let app = gio::Application::builder()
        .application_id(APPLICATION_ID)
        .build();

    let open = gio::SimpleAction::new("open-updates", None);
    open.connect_activate(|_, _| {
        let _ = Command::new("/usr/bin/vega-gtk")
            .env("VEGA_START_PAGE", "software")
            .spawn();
    });
    app.add_action(&open);

    app.connect_activate(|app| {
        let hold = app.hold();
        let app = app.clone();
        glib::MainContext::default().spawn_local(async move {
            let Ok(dbus) = VegaDbus::connect().await else {
                return;
            };
            let client = dbus.software();
            let Ok(mut events) = client.subscribe().await else {
                return;
            };

            if let Ok(status) = client.update_status().await {
                notify_if_needed(&app, status.total_count);
            }
            let polling_app = app.clone();
            glib::MainContext::default().spawn_local(async move {
                loop {
                    glib::timeout_future_seconds(60).await;
                    let Ok(dbus) = VegaDbus::connect().await else {
                        continue;
                    };
                    if let Ok(status) = dbus.software().update_status().await {
                        notify_if_needed(&polling_app, status.total_count);
                    }
                }
            });
            loop {
                match events.next().await {
                    Ok(SoftwareEvent::UpdatesAvailable(count)) => notify_if_needed(&app, count),
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            drop(hold);
        });
    });
    app.run()
}

fn notify_if_needed(app: &gio::Application, count: u32) {
    let state = notification_state_path();
    let previous = fs::read_to_string(&state)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok());
    if count == 0 {
        save_notified_count(&state, count);
        app.withdraw_notification("updates-available");
        return;
    }
    if !update_notifications_enabled() {
        app.withdraw_notification("updates-available");
        return;
    }
    if previous == Some(count) {
        return;
    }
    save_notified_count(&state, count);

    let notification = gio::Notification::new(&gettext("Atualizações disponíveis"));
    notification.set_body(Some(
        &gettext("{count} pacote(s) pendente(s)").replace("{count}", &count.to_string()),
    ));
    notification.set_icon(&gio::ThemedIcon::new("lyra-updates-symbolic"));
    notification.set_default_action("app.open-updates");
    app.send_notification(Some("updates-available"), &notification);
}

fn save_notified_count(path: &std::path::Path, count: u32) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{count}\n"));
}

fn notification_state_path() -> PathBuf {
    glib::user_data_dir().join("vega-gtk/update-notifier-count")
}

fn update_notifications_enabled() -> bool {
    let path = glib::user_config_dir().join("vega-gtk/preferences.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|value| value.get("notify_updates").and_then(Value::as_bool))
        .unwrap_or(true)
}
