//! Real GTK callbacks and D-Bus payloads, without a real daemon or user settings.
use super::*;
use glib::variant::ToVariant;
use std::{io::BufRead, process::Command, time::Duration};

type Calls = Rc<RefCell<Vec<(String, glib::Variant)>>>;

struct PrivateBus(std::process::Child);
impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn isolated_child() {
    let home = std::env::temp_dir().join(format!("vega-dialogs-{}", std::process::id()));
    std::fs::create_dir(&home).unwrap();
    let config = home.join("bus.conf");
    std::fs::write(&config, r#"<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>"#).unwrap();
    let mut bus = PrivateBus(
        Command::new("dbus-daemon")
            .arg("--config-file")
            .arg(&config)
            .args(["--nofork", "--print-address=1"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut address = String::new();
    std::io::BufReader::new(bus.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();

    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "application::dialog_tests::native_dialog_flows",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("VEGA_DIALOG_TEST_BUS", address.trim())
        .env("DBUS_SYSTEM_BUS_ADDRESS", address.trim())
        .env("DBUS_SESSION_BUS_ADDRESS", address.trim())
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("GSETTINGS_BACKEND", "memory")
        .env("GIO_USE_VFS", "local")
        .env("GTK_A11Y", "none")
        .status();
    std::fs::remove_dir_all(home).unwrap();
    assert!(result.unwrap().success());
}

fn fixture(address: &str, calls: &Calls) -> gio::DBusConnection {
    assert_eq!(std::env::var("DBUS_SYSTEM_BUS_ADDRESS").unwrap(), address);
    let connection = gio::DBusConnection::for_address_sync(
        address,
        gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
            | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
        None,
        gio::Cancellable::NONE,
    )
    .unwrap();
    connection
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "RequestName",
            Some(&("org.lyraos.Vega1", 0u32).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            2000,
            gio::Cancellable::NONE,
        )
        .unwrap();
    for (interface, methods) in [
        (
            "Backup",
            vec![
                ("ListConfigs", vec![], "a(sassss)"),
                ("CreateConfig", vec!["(sassss)"], "s"),
            ],
        ),
        (
            "Snapshots",
            vec![
                ("Available", vec![], "b"),
                ("ListSnapshots", vec![], "a(uxss)"),
                ("CreateSnapshot", vec!["s"], "u"),
                ("DiffPackagesLocalized", vec!["u", "s"], "as"),
                ("Rollback", vec!["u"], ""),
            ],
        ),
        (
            "Network",
            vec![
                ("ListInterfaces", vec![], "a(ssssssssssusb)"),
                ("SetStaticIpv4", vec!["s", "s", "s", "s"], ""),
                ("ConnectWifi", vec!["s", "s"], ""),
            ],
        ),
        (
            "Software",
            vec![
                ("AddRepo", vec!["s", "s"], "u"),
                ("Install", vec!["s", "s"], "u"),
                ("Remove", vec!["s", "s"], "u"),
                ("ClearCache", vec![], "u"),
                ("TrustRepoKey", vec!["s", "s"], "u"),
            ],
        ),
    ] {
        let mut xml = format!("<node><interface name='org.lyraos.Vega1.{interface}'>");
        for (method, inputs, output) in methods {
            xml.push_str(&format!("<method name='{method}'>"));
            for input in inputs {
                xml.push_str(&format!("<arg type='{input}' direction='in'/>"));
            }
            if !output.is_empty() {
                xml.push_str(&format!("<arg type='{output}' direction='out'/>"));
            }
            xml.push_str("</method>");
        }
        xml.push_str("</interface></node>");
        let info = gio::DBusNodeInfo::for_xml(&xml).unwrap();
        let calls = calls.clone();
        connection
            .register_object("/org/lyraos/Vega1", &info.interfaces()[0])
            .method_call(move |_, _, _, _, method, parameters, invocation| {
                let value = match method {
                    "ListConfigs" => Some(glib::Variant::parse(None, "(@a(sassss) [],)").unwrap()),
                    "Available" => Some((true,).to_variant()),
                    "ListSnapshots" => Some(glib::Variant::parse(None, "(@a(uxss) [],)").unwrap()),
                    "DiffPackagesLocalized" => {
                        Some((vec!["fixture-package: 2 -> 1"],).to_variant())
                    }
                    "ListInterfaces" => None,
                    _ => {
                        calls.borrow_mut().push((method.into(), parameters));
                        None
                    }
                };
                if let Some(value) = value {
                    invocation.return_value(Some(&value));
                } else {
                    // Record the exact request but never perform a mutation.
                    invocation.return_dbus_error("org.lyraos.Test.Rejected", "fixture rejected");
                }
            })
            .build()
            .unwrap();
    }
    connection
}

fn descendants(widget: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    let mut result = vec![widget.as_ref().clone()];
    let mut child = widget.as_ref().first_child();
    while let Some(current) = child {
        result.extend(descendants(&current));
        child = current.next_sibling();
    }
    result
}

async fn until(description: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out: {description}");
        glib::timeout_future(Duration::from_millis(10)).await;
    }
}

fn active_dialog() -> Option<adw::AlertDialog> {
    gtk::Window::list_toplevels()
        .iter()
        .flat_map(descendants)
        .filter_map(|widget| widget.downcast::<adw::AlertDialog>().ok())
        .find(|dialog| dialog.is_mapped())
}

async fn dialog(calls: &Calls, before: usize) -> adw::AlertDialog {
    until("form must be displayed before processing input", || {
        active_dialog().is_some()
    })
    .await;
    assert_eq!(calls.borrow().len(), before, "request before user response");
    active_dialog().unwrap()
}

async fn respond(dialog: &adw::AlertDialog, response: &str) {
    let label = dialog.response_label(response);
    let button = descendants(dialog)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button.label().as_deref() == Some(label.as_str()))
        .unwrap();
    button.emit_clicked();
    until("dialog closed", || !dialog.is_mapped()).await;
}

fn entries(dialog: &adw::AlertDialog) -> Vec<gtk::Entry> {
    descendants(&dialog.extra_child().unwrap())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Entry>().ok())
        .collect()
}

async fn request(calls: &Calls, before: usize, method: &str) -> glib::Variant {
    until(method, || calls.borrow().len() > before).await;
    assert_eq!(calls.borrow().len(), before + 1);
    let (actual, value) = calls.borrow()[before].clone();
    assert_eq!(actual, method);
    value
}

#[test]
#[ignore = "requires a graphical display and dbus-daemon; uses isolated settings and a fake daemon"]
fn native_dialog_flows() {
    let Ok(address) = std::env::var("VEGA_DIALOG_TEST_BUS") else {
        isolated_child();
        return;
    };
    adw::init().unwrap();
    let context = glib::MainContext::default();
    context.block_on(async {
        let calls: Calls = Rc::new(RefCell::new(Vec::new()));
        let _service = fixture(&address, &calls);
        let dbus = VegaDbus::connect().await.unwrap();
        let shell = VegaShell::new();
        let window = adw::ApplicationWindow::builder().content(&shell.root).build();
        configure_backup(&shell, dbus.clone());
        configure_snapshots(&shell, dbus.clone());
        configure_network(&shell, &window, dbus.clone());
        connect_add_repo(&shell.software, &dbus, &shell.dashboard_updates);
        until("initial reads", || shell.network.status.text().contains("fixture rejected")).await;
        until("snapshot availability", || shell.snapshots.create.is_sensitive()).await;

        for enabled in [true, false] {
            crate::preferences::save(&crate::preferences::Settings {
                confirm_actions: enabled, ..Default::default()
            });
            assert_eq!(crate::preferences::confirmations_enabled(), enabled);
            println!("Testing real GTK flows, confirm_actions={enabled}");

            // Cancel, close and invalid input must not create a backup.
            for response in ["cancel", "close", "create"] {
                let before = calls.borrow().len();
                shell.backup.new_config.emit_clicked();
                let d = dialog(&calls, before).await;
                if response == "close" {
                    d.close();
                    until("close treated as cancel", || !d.is_mapped()).await;
                } else {
                    respond(&d, response).await;
                }
                glib::timeout_future(Duration::from_millis(30)).await;
                assert_eq!(calls.borrow().len(), before);
                if response == "create" {
                    assert!(shell.backup.status.text().contains("obrigatórios"));
                }
            }
            let before = calls.borrow().len();
            shell.backup.new_config.emit_clicked();
            let d = dialog(&calls, before).await;
            for (entry, text) in entries(&d).iter().zip([" documents ", " /tmp/a, /tmp/b, ", " /tmp/dest ", " test-uuid "]) {
                entry.set_text(text);
            }
            descendants(&d).into_iter().find_map(|w| w.downcast::<gtk::DropDown>().ok()).unwrap().set_selected(2);
            respond(&d, "create").await;
            assert_eq!(request(&calls, before, "CreateConfig").await,
                (("documents", vec!["/tmp/a", "/tmp/b"], "/tmp/dest", "test-uuid", "weekly"),).to_variant());
            until("backup request finished", || shell.backup.new_config.is_sensitive()).await;

            let before = calls.borrow().len();
            shell.snapshots.create.emit_clicked();
            let d = dialog(&calls, before).await;
            entries(&d)[0].set_text(" before update ");
            respond(&d, "create").await;
            assert_eq!(request(&calls, before, "CreateSnapshot").await, ("before update",).to_variant());
            until("snapshot request finished", || shell.snapshots.create.is_sensitive()).await;

            shell.network.show_interfaces(&[lyra_vega_dbus::NetworkInterface {
                name: "test-connection".into(), kind: "ethernet".into(), state: "connected".into(),
                ipv4: String::new(), ipv6: String::new(), gateway: String::new(), dns: String::new(),
                mac: String::new(), speed: String::new(), ssid: String::new(), signal: 0,
                device: "test0".into(), autoconf: true,
            }]);
            shell.network.interfaces.select_row(shell.network.interfaces.row_at_index(0).as_ref());
            let before = calls.borrow().len();
            shell.network.interface_action.emit_clicked();
            let d = dialog(&calls, before).await;
            let fields = entries(&d);
            assert_eq!(fields[0].text(), "test-connection");
            fields[1].set_text("192.0.2.2/24");
            fields[2].set_text("192.0.2.1");
            fields[3].set_text("192.0.2.53");
            respond(&d, "apply").await;
            assert_eq!(request(&calls, before, "SetStaticIpv4").await,
                ("test-connection", "192.0.2.2/24", "192.0.2.1", "192.0.2.53").to_variant());
            until("IPv4 error reported", || shell.network.status.text().contains("fixture rejected")).await;

            shell.network.show_wifi(&[lyra_vega_dbus::WifiNetwork {
                ssid: "test-wifi".into(), security: "WPA2".into(), signal: 80,
                active: false, device: "test0".into(),
            }]);
            let before = calls.borrow().len();
            descendants(&shell.network.wifi).into_iter()
                .find(|w| w.has_css_class("wifi-row-action")).unwrap()
                .downcast::<gtk::Button>().unwrap().emit_clicked();
            let d = dialog(&calls, before).await;
            d.extra_child().unwrap().downcast::<gtk::PasswordEntry>().unwrap().set_text("fixture-password");
            respond(&d, "confirm").await;
            assert_eq!(request(&calls, before, "ConnectWifi").await,
                ("test-wifi", "fixture-password").to_variant());

            let before = calls.borrow().len();
            shell.software.add_repo_name.set_text("");
            shell.software.add_repo_url.set_text("");
            shell.software.add_repo_button.emit_clicked();
            assert_eq!(calls.borrow().len(), before);
            shell.software.add_repo_name.set_text(" test-repo ");
            shell.software.add_repo_url.set_text(" https://example.invalid/repo ");
            assert!(shell.software.add_repo_button.is_sensitive());
            shell.software.add_repo_button.emit_clicked();
            assert_eq!(request(&calls, before, "AddRepo").await,
                ("test-repo", "https://example.invalid/repo").to_variant());

            // All supported AI mutations still require an explicit decision.
            for (name, method) in [("install_package", "Install"), ("remove_package", "Remove"), ("clear_package_cache", "ClearCache")] {
                for approved in [false, true] {
                    let before = calls.borrow().len();
                    let page = shell.assistant.clone();
                    let dbus = dbus.clone();
                    let task = context.spawn_local(async move {
                        handle_assistant_mutation(&page, &dbus, &crate::assistant::ToolCall {
                            name: name.into(), input: serde_json::json!({"origin":"official", "id":"fixture-package"}),
                        }).await;
                    });
                    let d = dialog(&calls, before).await;
                    respond(&d, if approved {"confirm"} else {"cancel"}).await;
                    task.await.unwrap();
                    if approved {
                        request(&calls, before, method).await;
                    } else {
                        assert_eq!(calls.borrow().len(), before);
                    }
                }
            }

            // Rollback review shows the daemon's differences before applying.
            shell.snapshots.show_snapshots(vec![lyra_vega_dbus::Snapshot {
                id: 7, timestamp: 0, trigger: "manual".into(), description: "fixture".into(),
            }]);
            for approved in [false, true] {
                let before = calls.borrow().len();
                let button = descendants(&shell.snapshots.list).into_iter()
                    .filter_map(|w| w.downcast::<gtk::Button>().ok())
                    .find(|b| b.label().as_deref() == Some("Aplicar")).unwrap();
                button.emit_clicked();
                let d = dialog(&calls, before).await;
                let preview = descendants(&d).into_iter().find_map(|w| w.downcast::<gtk::TextView>().ok()).unwrap();
                let buffer = preview.buffer();
                assert_eq!(buffer.text(&buffer.start_iter(), &buffer.end_iter(), false), "fixture-package: 2 -> 1");
                respond(&d, if approved {"rollback"} else {"cancel"}).await;
                until("rollback finished", || button.is_sensitive()).await;
                if approved {
                    assert_eq!(request(&calls, before, "Rollback").await, (7u32,).to_variant());
                } else {
                    assert_eq!(calls.borrow().len(), before);
                }
            }
            // Trust decisions cannot be inherited from optional confirmations.
            for key_id in ["fixture-key", ""] {
                for approved in [false, true] {
                    let before = calls.borrow().len();
                    let page = shell.software.clone();
                    let label = shell.dashboard_updates.clone();
                    let client = dbus.software();
                    let task = context.spawn_local(async move {
                        confirm_and_trust_repo_key(&page, &client, RepositoryKeyInfo {
                            transaction_id: 1, repo: "fixture-repo".into(), key_id: key_id.into(),
                            fingerprint: "ABCD 1234".into(), user_id: "Fixture signer".into(),
                        }, &label).await;
                    });
                    let d = dialog(&calls, before).await;
                    assert!(d.body().contains(if key_id.is_empty() { "sem verificação de assinatura" } else { "ABCD 1234" }));
                    respond(&d, if approved {"confirm"} else {"cancel"}).await;
                    task.await.unwrap();
                    if approved {
                        assert_eq!(request(&calls, before, "TrustRepoKey").await, ("fixture-repo", key_id).to_variant());
                    } else {
                        assert_eq!(calls.borrow().len(), before);
                    }
                }
            }

            // Plain confirmations still honor the preference.
            let d = adw::AlertDialog::new(Some("Already specified action"), None);
            d.add_responses(&[("cancel", "Cancel"), ("confirm", "Continue")]);
            d.set_close_response("cancel");
            let copy = d.clone();
            let task = context.spawn_local(async move { confirm_dialog(&copy, "confirm").await });
            if enabled {
                until("optional confirmation enabled", || d.is_mapped()).await;
                respond(&d, "cancel").await;
                assert!(!task.await.unwrap());
            } else {
                assert!(task.await.unwrap());
                assert!(!d.is_mapped());
            }
        }
        window.destroy();
    });
}
