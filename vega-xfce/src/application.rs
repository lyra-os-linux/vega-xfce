use crate::i18n::gettext;
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use crate::model::AppIdentity;
use crate::ui::VegaShell;
use lyra_vega_dbus::{
    BackupClient, BackupConfig, BackupEvent, BluetoothClient, DateTimeClient, FirewallClient,
    HardwareClient, KernelClient, LogsClient, MonitorClient, NetworkClient, PackageRef,
    RepositoryKeyInfo, ServicesClient, SnapshotsClient, SoftwareClient, SoftwareEvent,
    StorageClient, SystemClient, UsersClient, VegaDbus,
};

pub const APPLICATION_ID: &str = "org.lyraos.Vega.Xfce";

thread_local! {
    static LAST_STRUGGLING_SERVICES: Cell<Option<usize>> = const { Cell::new(None) };
}

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .build();
    let current_window: Rc<RefCell<Option<(VegaShell, adw::ApplicationWindow)>>> =
        Rc::new(RefCell::new(None));
    let open_updates = Rc::new(Cell::new(false));

    let action = gio::SimpleAction::new("open-updates", None);
    let action_app = app.clone();
    let action_request = open_updates.clone();
    action.connect_activate(move |_, _| {
        action_request.set(true);
        action_app.activate();
    });
    app.add_action(&action);

    app.connect_startup(|_| install_style());
    let started = Instant::now();
    let activate_window = current_window.clone();
    let activate_request = open_updates.clone();
    app.connect_activate(move |app| {
        if let Some((shell, window)) = activate_window.borrow().as_ref() {
            if activate_request.replace(false) {
                shell.stack.set_visible_child_name("software");
                shell.software.select_updates();
            }
            window.present();
            return;
        }
        let show_updates = activate_request.replace(false)
            || std::env::var_os("VEGA_START_PAGE").as_deref()
                == Some(std::ffi::OsStr::new("software"));
        let window = build_window(app, show_updates);
        *activate_window.borrow_mut() = Some(window);
        if std::env::var_os("VEGA_BENCHMARK_MARKER").is_some() {
            eprintln!("VEGA_WINDOW_READY_MS={}", started.elapsed().as_millis());
        }
    });
    app.run()
}

fn install_style() {
    gtk::Window::set_default_icon_name("vega");
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_str!("../resources/style.css"));
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("display gráfico disponível"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_window(app: &adw::Application, show_updates: bool) -> (VegaShell, adw::ApplicationWindow) {
    let identity = AppIdentity::default();
    let shell = VegaShell::new();

    if show_updates {
        shell.stack.set_visible_child_name("software");
        shell.software.select_updates();
    }

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(&format!("{} — XFCE", identity.name))
        .default_width(1180)
        .default_height(760)
        .content(&shell.root)
        .build();
    window.set_icon_name(Some("vega"));

    update_content(shell.clone(), window.clone());
    window.present();
    (shell, window)
}

fn update_content(shell: VegaShell, window: adw::ApplicationWindow) {
    glib::MainContext::default().spawn_local(async move {
        let dbus = match VegaDbus::connect().await {
            Ok(dbus) => dbus,
            Err(error) => {
                set_unavailable(&shell, &error.to_string());
                return;
            }
        };

        // Cada página é configurada (e seu carregamento inicial disparado) de forma
        // independente, sem esperar a cadeia sequencial do resumo do painel abaixo
        // terminar — do contrário, navegar para outra página antes do painel concluir
        // deixa essa página presa em "carregando" indefinidamente.
        configure_nvidia(&shell, dbus.clone());
        configure_non_free_firmware(&shell, dbus.clone());
        configure_software(&shell, &window, dbus.clone());
        configure_backup(&shell, dbus.clone());
        configure_snapshots(&shell, dbus.clone());
        configure_kernel(&shell, dbus.clone());
        configure_datetime(&shell, dbus.clone());
        configure_screen(&shell, &window, dbus.clone());
        configure_storage(&shell, dbus.clone());
        configure_network(&shell, &window, dbus.clone());
        configure_bluetooth(&shell, &window, dbus.clone());
        configure_services(&shell, dbus.clone());
        configure_users(&shell, dbus.clone());
        configure_logs(&shell, dbus.clone());
        configure_assistant(&shell, &window, dbus.clone());

        refresh_dashboard_summary(&shell, &dbus).await;
        schedule_dashboard_refresh(shell, dbus);
    });
}

fn schedule_dashboard_refresh(shell: VegaShell, dbus: VegaDbus) {
    let elapsed_minutes = Rc::new(std::cell::Cell::new(0_u32));
    glib::timeout_add_seconds_local(60, move || {
        if shell.root.root().is_none() {
            return glib::ControlFlow::Break;
        }
        let elapsed = elapsed_minutes.get() + 1;
        let interval = crate::preferences::refresh_interval_minutes();
        if elapsed >= interval {
            elapsed_minutes.set(0);
            let shell = shell.clone();
            let dbus = dbus.clone();
            glib::MainContext::default().spawn_local(async move {
                refresh_dashboard_summary(&shell, &dbus).await;
            });
        } else {
            elapsed_minutes.set(elapsed);
        }
        glib::ControlFlow::Continue
    });
}

// Cada bloco abaixo é independente dos outros (nenhum lê o resultado de
// outro), então são todos disparados de uma vez com join! em vez de
// aguardados um atrás do outro — o tempo total do painel passa a ser o da
// chamada mais lenta (tipicamente list_updates, que shella pro zypper) em
// vez da soma de todas elas.
async fn refresh_dashboard_summary(shell: &VegaShell, dbus: &VegaDbus) {
    let status_task = async {
        match dbus.system().status().await {
            Ok(status) => {
                shell.backend_status.set_label(
                    &gettext("vegad {version} conectado • {distro}")
                        .replace("{version}", &status.version)
                        .replace("{distro}", &status.distro),
                );
                shell.dashboard_system.set_label(
                    &gettext("{distro} • interface nativa ativa")
                        .replace("{distro}", &status.distro),
                );
            }
            Err(error) => set_unavailable(shell, &error.to_string()),
        }
    };

    let hardware_task = async {
        match dbus.hardware().inventory().await {
            Ok(inventory) => {
                shell.hardware_cpu.set_label(&inventory.cpu);
                shell.hardware_gpu.set_label(&inventory.gpu);
                shell.hardware_ram.set_label(&inventory.ram);
            }
            Err(error) => {
                let message = error.to_string();
                shell.hardware_cpu.set_label(&message);
                shell.hardware_gpu.set_label("—");
                shell.hardware_ram.set_label("—");
            }
        }
    };

    let firmware_task = async {
        match dbus.hardware().firmware_status().await {
            Ok(status) => shell.hardware_firmware.set_label(&status),
            Err(error) => shell.hardware_firmware.set_label(&error.to_string()),
        }
    };

    let software_client = dbus.software();
    let updates_task = refresh_dashboard_updates(&shell.dashboard_updates, &software_client);

    let backup_task = async {
        match dbus.backup().list_configs().await {
            Ok(configs) if configs.is_empty() => shell
                .dashboard_backup
                .set_label(&gettext("Não configurado")),
            Ok(configs) => shell.dashboard_backup.set_label(
                &gettext("{count} destino(s) configurado(s)")
                    .replace("{count}", &configs.len().to_string()),
            ),
            Err(error) => shell.dashboard_backup.set_label(&error.to_string()),
        }
    };

    let snapshots_task = async {
        match dbus.snapshots().available().await {
            Ok(false) => shell
                .dashboard_snapshots
                .set_label(&gettext("Não suportado neste sistema")),
            Ok(true) => match dbus.snapshots().list().await {
                Ok(snapshots) if snapshots.is_empty() => shell
                    .dashboard_snapshots
                    .set_label(&gettext("Nenhum snapshot")),
                Ok(snapshots) => shell.dashboard_snapshots.set_label(
                    &gettext("{count} snapshot(s)")
                        .replace("{count}", &snapshots.len().to_string()),
                ),
                Err(error) => shell.dashboard_snapshots.set_label(&error.to_string()),
            },
            Err(error) => shell.dashboard_snapshots.set_label(&error.to_string()),
        }
    };

    let services_task = async {
        match dbus.services().list().await {
            Ok(services) => {
                let struggling = services
                    .iter()
                    .filter(|service| service.available && service.enabled && !service.active)
                    .count();
                LAST_STRUGGLING_SERVICES.with(|previous| {
                    if previous.get().is_some_and(|old| old != struggling)
                        && struggling > 0
                        && crate::preferences::notifications().1
                    {
                        send_notification(
                            &gettext("Falha em serviços"),
                            &gettext("{count} serviço(s) requer(em) atenção")
                                .replace("{count}", &struggling.to_string()),
                            "service-failures",
                        );
                    }
                    previous.set(Some(struggling));
                });
                shell.dashboard_services.set_label(&if struggling == 0 {
                    gettext("Nenhum serviço com problema")
                } else {
                    gettext("{count} serviço(s) habilitado(s), mas parado(s)")
                        .replace("{count}", &struggling.to_string())
                });
            }
            Err(error) => shell.dashboard_services.set_label(&error.to_string()),
        }
    };

    let disk_task = async {
        match dbus.system().disk_usage().await {
            Ok((used, total, percent)) => shell.dashboard_disk.set_label(
                &gettext("{percent}% • {used} de {total} usados")
                    .replace("{percent}", &percent.to_string())
                    .replace("{used}", &used)
                    .replace("{total}", &total),
            ),
            Err(error) => shell.dashboard_disk.set_label(&error.to_string()),
        }
    };

    futures_util::join!(
        status_task,
        hardware_task,
        firmware_task,
        updates_task,
        backup_task,
        snapshots_task,
        services_task,
        disk_task,
    );
}

fn configure_assistant(shell: &VegaShell, window: &adw::ApplicationWindow, dbus: VegaDbus) {
    let page = shell.assistant.clone();
    page.status
        .set_label(&if crate::assistant::keyring_available() {
            gettext("Pronto • credenciais protegidas pelo Secret Service")
        } else {
            gettext("Secret Service indisponível: não será possível armazenar chaves")
        });

    let provider_page = page.clone();
    page.provider.connect_selected_notify(move |_| {
        let settings = crate::assistant::load_settings();
        let provider = crate::assistant::Provider::from_index(provider_page.provider.selected());
        let model = match provider {
            crate::assistant::Provider::Anthropic => settings.anthropic_model,
            crate::assistant::Provider::OpenAi => settings.openai_model,
            crate::assistant::Provider::Gemini => settings.gemini_model,
        };
        provider_page.show_models(vec![model.clone()], &model);
        provider_page.api_key.set_text("");
        let page = provider_page.clone();
        glib::MainContext::default().spawn_local(async move {
            refresh_assistant_models(&page).await;
        });
    });

    let settings_page = page.clone();
    page.save_settings.connect_clicked(move |_| {
        match crate::assistant::save_settings(&settings_page.settings()) {
            Ok(()) => settings_page
                .status
                .set_label(&gettext("Configurações salvas")),
            Err(error) => settings_page.status.set_label(&error.to_string()),
        }
    });

    let key_page = page.clone();
    page.save_key.connect_clicked(move |_| {
        let provider = crate::assistant::Provider::from_index(key_page.provider.selected());
        let key = key_page.api_key.text().to_string();
        let page = key_page.clone();
        glib::MainContext::default().spawn_local(async move {
            page.status
                .set_label(&gettext("Salvando chave no keyring…"));
            let result =
                gio::spawn_blocking(move || crate::assistant::save_key(provider, &key)).await;
            match result {
                Ok(Ok(())) => {
                    page.api_key.set_text("");
                    page.status
                        .set_label(&gettext("Chave salva com segurança no keyring"));
                    refresh_assistant_models(&page).await;
                }
                Ok(Err(error)) => page.status.set_label(&error.to_string()),
                Err(_) => page
                    .status
                    .set_label(&gettext("Falha interna ao acessar o keyring")),
            }
        });
    });

    let models_page = page.clone();
    page.refresh_models.connect_clicked(move |_| {
        let page = models_page.clone();
        glib::MainContext::default().spawn_local(async move {
            refresh_assistant_models(&page).await;
        });
    });

    let remove_page = page.clone();
    page.remove_key.connect_clicked(move |_| {
        let provider = crate::assistant::Provider::from_index(remove_page.provider.selected());
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Remover chave de API?")),
            Some(
                &gettext("A chave de {provider} será apagada do keyring.")
                    .replace("{provider}", provider.label()),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("remove", &gettext("Remover")),
        ]);
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = remove_page.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "remove").await {
                return;
            }
            let result = gio::spawn_blocking(move || crate::assistant::clear_key(provider)).await;
            match result {
                Ok(Ok(())) => page.status.set_label(&gettext("Chave removida do keyring")),
                Ok(Err(error)) => page.status.set_label(&error.to_string()),
                Err(_) => page
                    .status
                    .set_label(&gettext("Falha interna ao acessar o keyring")),
            }
        });
    });

    let clear_page = page.clone();
    page.clear_history.connect_clicked(move |_| {
        clear_page.clear();
        match crate::assistant::clear_history() {
            Ok(()) => clear_page.status.set_label(&gettext("Conversa limpa")),
            Err(error) => clear_page.status.set_label(&error.to_string()),
        }
    });

    let attach_page = page.clone();
    let attach_window = window.clone();
    page.attach.connect_clicked(move |_| {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&gettext("Imagens e arquivos de texto")));
        filter.add_mime_type("image/*");
        filter.add_mime_type("text/*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Anexar arquivo ou imagem"))
            .filters(&filters)
            .build();
        let page = attach_page.clone();
        let window = attach_window.clone();
        glib::MainContext::default().spawn_local(async move {
            let Ok(files) = dialog.open_multiple_future(Some(&window)).await else {
                return;
            };
            for index in 0..files.n_items() {
                let Some(file) = files.item(index).and_downcast::<gio::File>() else {
                    continue;
                };
                let Some(path) = file.path() else {
                    page.status
                        .set_label(&gettext("Só é possível anexar arquivos locais."));
                    continue;
                };
                let result =
                    gio::spawn_blocking(move || crate::assistant::read_attachment(&path)).await;
                match result {
                    Ok(Ok(attachment)) => page.stage_attachment(attachment),
                    Ok(Err(error)) => page.status.set_label(&error.to_string()),
                    Err(_) => page
                        .status
                        .set_label(&gettext("Falha interna ao ler o arquivo")),
                }
            }
        });
    });

    connect_assistant_send(&page.send, &page, &dbus);
    let initial_page = page.clone();
    glib::MainContext::default().spawn_local(async move {
        refresh_assistant_models(&initial_page).await;
    });
}

async fn refresh_assistant_models(page: &crate::ui::AssistantPage) {
    let provider = crate::assistant::Provider::from_index(page.provider.selected());
    let selected = page.selected_model();
    page.refresh_models.set_sensitive(false);
    page.status.set_label(
        &gettext("Consultando modelos da {provider}…").replace("{provider}", provider.label()),
    );
    let result = gio::spawn_blocking(move || crate::assistant::list_models(provider)).await;
    match result {
        Ok(Ok(models)) => {
            page.show_models(models, &selected);
            page.status.set_label(
                &gettext("{count} modelo(s) compatível(is) disponível(is)").replace(
                    "{count}",
                    &page
                        .model
                        .model()
                        .map(|model| model.n_items())
                        .unwrap_or(0)
                        .to_string(),
                ),
            );
        }
        Ok(Err(error)) => page.status.set_label(&error.to_string()),
        Err(_) => page
            .status
            .set_label(&gettext("Falha interna ao consultar os modelos")),
    }
    page.refresh_models.set_sensitive(true);
}

fn connect_assistant_send(button: &gtk::Button, page: &crate::ui::AssistantPage, dbus: &VegaDbus) {
    let page = page.clone();
    let dbus = dbus.clone();
    button.connect_clicked(move |_| {
        let prompt = page.prompt_text();
        if prompt.is_empty() && !page.has_staged_attachments() {
            return;
        }
        page.clear_prompt();
        let attachments = page.take_staged_attachments();
        for attachment in &attachments {
            let _ = crate::assistant::audit("user_attachment", &attachment.name);
        }
        page.append("user", prompt.clone(), attachments);
        let _ = crate::assistant::audit("user_message", &prompt);
        let _ = crate::assistant::save_history(&page.history());
        let settings = page.settings();
        if let Err(error) = crate::assistant::save_settings(&settings) {
            page.status.set_label(&error.to_string());
            return;
        }
        let history = page.history();
        let request_page = page.clone();
        let request_dbus = dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            request_page.set_busy(true);
            request_page
                .status
                .set_label(&gettext("Consultando o provedor…"));
            let mut history = history;
            let mut input_tokens = 0;
            let mut output_tokens = 0;
            let mut estimated_cost = 0.0;
            let mut has_cost = false;
            let max_rounds = settings.max_rounds_per_message.clamp(1, 20);
            for round in 0..max_rounds {
                request_page.status.set_label(
                    &gettext("Pensando… etapa {step} de {total}")
                        .replace("{step}", &(round + 1).to_string())
                        .replace("{total}", &max_rounds.to_string()),
                );
                let round_settings = settings.clone();
                let round_history = history.clone();
                let result = gio::spawn_blocking(move || {
                    if round == 0 {
                        crate::assistant::send(&round_settings, &round_history)
                    } else {
                        crate::assistant::continue_after_tool(&round_settings, &round_history)
                    }
                })
                .await;
                let reply = match result {
                    Ok(Ok(reply)) => reply,
                    Ok(Err(error)) => {
                        request_page.status.set_label(&error.to_string());
                        break;
                    }
                    Err(_) => {
                        request_page
                            .status
                            .set_label(&gettext("Falha interna ao consultar o provedor"));
                        break;
                    }
                };
                input_tokens += reply.input_tokens;
                output_tokens += reply.output_tokens;
                if let Some(cost) = reply.estimated_cost_usd {
                    estimated_cost += cost;
                    has_cost = true;
                }
                if !reply.text.is_empty() {
                    request_page.append_progressively(reply.text.clone()).await;
                    let _ = crate::assistant::audit("assistant_message", &reply.text);
                }
                let has_tools = !reply.tool_calls.is_empty();
                for tool_call in reply.tool_calls {
                    handle_assistant_tool(&request_page, &request_dbus, tool_call).await;
                }
                history = request_page.history();
                let _ = crate::assistant::save_history(&history);
                let cost = if has_cost {
                    gettext(" • estimativa US$ {value}")
                        .replace("{value}", &format!("{estimated_cost:.6}"))
                } else {
                    String::new()
                };
                request_page.status.set_label(
                    &gettext("{input} tokens de entrada • {output} de saída{cost}")
                        .replace("{input}", &input_tokens.to_string())
                        .replace("{output}", &output_tokens.to_string())
                        .replace("{cost}", &cost),
                );
                if !has_tools {
                    break;
                }
            }
            request_page.set_busy(false);
        });
    });
}

async fn handle_assistant_tool(
    page: &crate::ui::AssistantPage,
    dbus: &VegaDbus,
    call: crate::assistant::ToolCall,
) {
    let result = match call.name.as_str() {
        "search_packages" => {
            let query = tool_string(&call.input, "query");
            dbus.software()
                .search(&query)
                .await
                .map(|packages| {
                    packages
                        .into_iter()
                        .take(20)
                        .map(|package| {
                            format!(
                                "{} • {} • {}",
                                package.id, package.origin, package.description
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .map_err(|error| error.to_string())
        }
        "list_available_updates" => dbus
            .software()
            .list_updates()
            .await
            .map(|packages| {
                packages
                    .into_iter()
                    .map(|package| format!("{} • {}", package.id, package.origin))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .map_err(|error| error.to_string()),
        "get_recent_logs" => {
            let unit = tool_string(&call.input, "unit");
            let priority = tool_string(&call.input, "priority");
            let max_lines = tool_u32(&call.input, "max_lines", 50).clamp(1, 200);
            dbus.logs()
                .query(&unit, &priority, "", "", max_lines)
                .await
                .map(|lines| {
                    let logs = lines.join("\n");
                    if crate::preferences::redact_ai_data() {
                        crate::assistant::redact(&logs)
                    } else {
                        logs
                    }
                })
                .map_err(|error| error.to_string())
        }
        "get_system_status" => {
            let status = dbus
                .system()
                .status()
                .await
                .map_err(|error| error.to_string());
            let disk = dbus
                .system()
                .disk_usage()
                .await
                .map_err(|error| error.to_string());
            match (status, disk) {
                (Ok(status), Ok((used, total, percent))) => {
                    page.append(
                        "assistant",
                        gettext("Sistema: {distro} • vegad {version}\nDisco: {used} de {total} ({percent}%)")
                            .replace("{distro}", &status.distro)
                            .replace("{version}", &status.version)
                            .replace("{used}", &used)
                            .replace("{total}", &total)
                            .replace("{percent}", &percent.to_string()),
                        Vec::new(),
                    );
                    let _ = crate::assistant::audit("read", "get_system_status concluída");
                    return;
                }
                (Err(error), _) | (_, Err(error)) => {
                    page.status.set_label(&error);
                    return;
                }
            }
        }
        name if crate::assistant::is_mutating_tool(name) => {
            handle_assistant_mutation(page, dbus, &call).await;
            return;
        }
        _ => {
            page.status.set_label(
                &gettext("Ferramenta desconhecida recusada: {name}").replace("{name}", &call.name),
            );
            let _ = crate::assistant::audit("tool_rejected", &call.name);
            return;
        }
    };
    match result {
        Ok(output) => {
            let output = if output.is_empty() {
                gettext("Nenhum resultado.")
            } else {
                output
            };
            let instruction = gettext("Continue a resposta usando este resultado.");
            page.append(
                "user",
                format!(
                    "<dado_nao_confiavel origem=\"tool:{}\">\n{output}\n</dado_nao_confiavel>\n{instruction}",
                    call.name
                ),
                Vec::new(),
            );
            let _ = crate::assistant::audit("read", &format!("{} concluída", call.name));
        }
        Err(error) => {
            page.status.set_label(&error.to_string());
            let _ = crate::assistant::audit("read_error", &format!("{}: {error}", call.name));
        }
    }
}

async fn handle_assistant_mutation(
    page: &crate::ui::AssistantPage,
    dbus: &VegaDbus,
    call: &crate::assistant::ToolCall,
) {
    let origin = tool_string(&call.input, "origin");
    let id = tool_string(&call.input, "id");
    if call.name != "clear_package_cache" && id.is_empty() {
        page.status
            .set_label(&gettext("Proposta recusada: pacote sem identificador"));
        return;
    }
    if call.name == "install_package" && !crate::assistant::install_origin_allowed(&origin) {
        page.status.set_label(&gettext(
            "Esta origem não pode ser instalada pelo Assistente; use a tela Software para revisão",
        ));
        return;
    }
    let (title, description, confirm, destructive) = match call.name.as_str() {
        "install_package" => (
            gettext("Instalar pacote?"),
            gettext("Instalar {id} da origem {origin}.")
                .replace("{id}", &id)
                .replace("{origin}", &origin),
            gettext("Instalar"),
            false,
        ),
        "remove_package" => (
            gettext("Remover pacote?"),
            gettext("Remover {id} da origem {origin}.")
                .replace("{id}", &id)
                .replace("{origin}", &origin),
            gettext("Remover"),
            true,
        ),
        _ => (
            gettext("Limpar cache?"),
            gettext("Remover os pacotes baixados do cache."),
            gettext("Limpar"),
            true,
        ),
    };
    let _ = crate::assistant::audit("mutation_proposed", &description);
    let dialog = adw::AlertDialog::new(Some(&title), Some(&description));
    dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &confirm)]);
    dialog.set_response_appearance(
        "confirm",
        if destructive {
            adw::ResponseAppearance::Destructive
        } else {
            adw::ResponseAppearance::Suggested
        },
    );
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    if !confirm_dialog(&dialog, "confirm").await {
        page.append(
            "assistant",
            gettext("A proposta foi rejeitada. Nenhuma alteração foi realizada."),
            Vec::new(),
        );
        let _ = crate::assistant::audit("mutation_rejected", &description);
        return;
    }
    let result = match call.name.as_str() {
        "install_package" => dbus.software().install(&origin, &id).await,
        "remove_package" => dbus.software().remove(&origin, &id).await,
        _ => dbus.software().clear_cache().await,
    };
    match result {
        Ok(transaction) => {
            page.append(
                "assistant",
                gettext("Ação aprovada e enviada ao vegad (transação #{transaction}).")
                    .replace("{transaction}", &transaction.to_string()),
                Vec::new(),
            );
            let _ = crate::assistant::audit("mutation_approved", &description);
        }
        Err(error) => {
            page.status.set_label(&error.to_string());
            let _ = crate::assistant::audit("mutation_error", &format!("{description}: {error}"));
        }
    }
}

fn tool_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn tool_u32(value: &serde_json::Value, key: &str, default: u32) -> u32 {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(default)
}

fn configure_logs(shell: &VegaShell, dbus: VegaDbus) {
    let page = shell.logs.clone();
    // O journal administrativo exige polkit. Não consulte na inicialização do
    // Vega: isso faria o agente pedir senha antes de qualquer ação do usuário.
    // O primeiro carregamento acontece somente quando o módulo de logs fica
    // visível; consultas seguintes são acionadas pelos controles da página.
    let loaded = Rc::new(Cell::new(false));
    let load_page = page.clone();
    let load_dbus = dbus.clone();
    let load_once = loaded.clone();
    page.root.connect_map(move |_| {
        if load_once.replace(true) {
            return;
        }
        let page = load_page.clone();
        let dbus = load_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            match dbus.logs().list_units().await {
                Ok(units) => page.show_units(&units),
                Err(error) => {
                    page.status.set_label(&error.to_string());
                    return;
                }
            }
            refresh_logs_page(&page, &dbus).await;
        });
    });

    let query_page = page.clone();
    let query_dbus = dbus.clone();
    page.query.connect_clicked(move |_| {
        let page = query_page.clone();
        let dbus = query_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            refresh_logs_page(&page, &dbus).await;
        });
    });
    let search_page = page.clone();
    let search_dbus = dbus.clone();
    page.search.connect_activate(move |_| {
        let page = search_page.clone();
        let dbus = search_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            refresh_logs_page(&page, &dbus).await;
        });
    });
}

async fn refresh_logs_page(page: &crate::ui::LogsPage, dbus: &VegaDbus) {
    page.set_busy(true);
    page.status.set_label(&gettext("Consultando o journal…"));
    let result = dbus
        .logs()
        .query(
            &page.selected_unit(),
            page.selected_priority(),
            page.selected_since(),
            page.search.text().trim(),
            page.selected_limit(),
        )
        .await;
    match result {
        Ok(lines) => page.show_lines(&lines),
        Err(error) => page.status.set_label(&error.to_string()),
    }
    page.set_busy(false);
}

fn configure_users(shell: &VegaShell, dbus: VegaDbus) {
    let page = shell.users.clone();
    let load_page = page.clone();
    let load_dbus = dbus.clone();
    glib::MainContext::default().spawn_local(async move {
        refresh_users_page(&load_page, &load_dbus).await;
    });

    let new_page = page.clone();
    page.open_create
        .connect_clicked(move |_| new_page.begin_create());

    let photo_page = page.clone();
    page.photo.connect_clicked(move |_| {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&gettext("Imagens PNG ou JPEG")));
        filter.add_mime_type("image/png");
        filter.add_mime_type("image/jpeg");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let chooser = gtk::FileDialog::builder()
            .title(gettext("Selecionar foto"))
            .accept_label(gettext("Selecionar"))
            .filters(&filters)
            .default_filter(&filter)
            .modal(true)
            .build();
        let page = photo_page.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(file) = chooser.open_future(gtk::Window::NONE).await
                && let Some(path) = file.path()
            {
                match std::fs::read(&path) {
                    Ok(bytes) if bytes.len() <= 5 * 1024 * 1024 => {
                        *page.photo_data.borrow_mut() = bytes;
                        page.photo.set_label(
                            &path
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| gettext("Foto selecionada")),
                        );
                    }
                    Ok(_) => page
                        .status
                        .set_label(&gettext("A foto deve ter no máximo 5 MB.")),
                    Err(error) => page.status.set_label(
                        &gettext("Não foi possível ler a foto: {error}")
                            .replace("{error}", &error.to_string()),
                    ),
                }
            }
        });
    });

    let create_page = page.clone();
    let create_dbus = dbus.clone();
    page.create.connect_clicked(move |_| {
        let username = create_page.username.text().trim().to_owned();
        let full_name = create_page.full_name.text().trim().to_owned();
        let password = create_page.password.text().to_string();
        let groups = create_page
            .groups
            .text()
            .split(',')
            .map(str::trim)
            .filter(|group| !group.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let photo = create_page.photo_data.borrow().clone();
        let editing = create_page.editing.borrow().is_some();
        let is_admin = create_page.admin.is_active();
        let role = if is_admin {
            gettext("administrador")
        } else {
            gettext("usuário comum")
        };
        let title = if editing {
            gettext("Salvar alterações?")
        } else {
            gettext("Criar usuário?")
        };
        let message = if editing {
            gettext("Os dados da conta {username} serão atualizados.")
                .replace("{username}", &username)
        } else {
            gettext("A conta {username} será criada como {role}.")
                .replace("{username}", &username)
                .replace("{role}", &role)
        };
        let dialog = adw::AlertDialog::new(Some(&title), Some(&message));
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("confirm", &gettext("Criar")),
        ]);
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = create_page.clone();
        let dbus = create_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.set_busy(true);
            page.status
                .set_label(&gettext("Criando {username}…").replace("{username}", &username));
            let result = if editing {
                dbus.users()
                    .update(&username, &full_name, &password, &groups, &photo, is_admin)
                    .await
            } else {
                dbus.users()
                    .create(&username, &full_name, &password, &groups, &photo, is_admin)
                    .await
            };
            match result {
                Ok(()) => {
                    page.finish_edit();
                    page.username.set_text("");
                    page.full_name.set_text("");
                    page.password.set_text("");
                    page.password_confirm.set_text("");
                    page.groups.set_text("");
                    page.photo_data.borrow_mut().clear();
                    page.photo.set_label(&gettext("Selecionar foto…"));
                    page.editor_dialog.close();
                    refresh_users_page(&page, &dbus).await;
                }
                Err(error) => page.status.set_label(&error.to_string()),
            }
            page.set_busy(false);
        });
    });

    let edit_page = page.clone();
    page.edit.connect_clicked(move |_| {
        if let Some(user) = edit_page.selected() {
            edit_page.begin_edit(&user);
        }
    });

    connect_user_action(&page.change_admin, &page, &dbus, UserAction::Admin);
    connect_user_action(&page.remove, &page, &dbus, UserAction::Remove);
}

#[derive(Clone, Copy)]
enum UserAction {
    Admin,
    Remove,
}

fn connect_user_action(
    button: &gtk::Button,
    page: &crate::ui::UsersPage,
    dbus: &VegaDbus,
    action: UserAction,
) {
    let page = page.clone();
    let dbus = dbus.clone();
    button.connect_clicked(move |_| {
        let Some(user) = page.selected() else {
            return;
        };
        let (title, message, confirm) = match action {
            UserAction::Admin if user.is_admin => (
                gettext("Remover privilégios administrativos?"),
                gettext("{user} deixará de administrar o sistema.")
                    .replace("{user}", &user.username),
                gettext("Remover admin"),
            ),
            UserAction::Admin => (
                gettext("Conceder privilégios administrativos?"),
                gettext("{user} poderá administrar o sistema.").replace("{user}", &user.username),
                gettext("Tornar admin"),
            ),
            UserAction::Remove => (
                gettext("Remover usuário?"),
                gettext("A conta {user} e seu diretório pessoal serão removidos.")
                    .replace("{user}", &user.username),
                gettext("Remover"),
            ),
        };
        let dialog = adw::AlertDialog::new(Some(&title), Some(&message));
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &confirm)]);
        dialog.set_response_appearance(
            "confirm",
            if matches!(action, UserAction::Remove) {
                adw::ResponseAppearance::Destructive
            } else {
                adw::ResponseAppearance::Suggested
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = page.clone();
        let dbus = dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.set_busy(true);
            page.status
                .set_label(&gettext("Processando {user}…").replace("{user}", &user.username));
            let result = match action {
                UserAction::Admin => dbus.users().set_admin(&user.username, !user.is_admin).await,
                UserAction::Remove => dbus.users().remove(&user.username).await,
            };
            match result {
                Ok(()) => refresh_users_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
            page.set_busy(false);
        });
    });
}

async fn refresh_users_page(page: &crate::ui::UsersPage, dbus: &VegaDbus) {
    page.status.set_label(&gettext("Carregando usuários…"));
    match dbus.users().list().await {
        Ok(users) => page.show(users),
        Err(error) => page.status.set_label(&error.to_string()),
    }
}

fn configure_services(shell: &VegaShell, dbus: VegaDbus) {
    let page = shell.services.clone();
    let load_page = page.clone();
    let load_dbus = dbus.clone();
    glib::MainContext::default().spawn_local(async move {
        refresh_services_page(&load_page, &load_dbus, false).await;
    });
    let curated_page = page.clone();
    let curated_dbus = dbus.clone();
    page.curated.connect_clicked(move |button| {
        if !button.is_active() {
            return;
        }
        let page = curated_page.clone();
        let dbus = curated_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            refresh_services_page(&page, &dbus, false).await;
        });
    });
    let all_page = page.clone();
    let all_dbus = dbus.clone();
    page.all.connect_clicked(move |button| {
        if !button.is_active() {
            return;
        }
        let page = all_page.clone();
        let dbus = all_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            refresh_services_page(&page, &dbus, true).await;
        });
    });
    connect_service_action(&page.enable, &page, &dbus, ServiceAction::Enable);
    connect_service_action(&page.running, &page, &dbus, ServiceAction::Running);
    connect_service_action(&page.restart, &page, &dbus, ServiceAction::Restart);
}

#[derive(Clone, Copy)]
enum ServiceAction {
    Enable,
    Running,
    Restart,
}

fn connect_service_action(
    button: &gtk::Button,
    page: &crate::ui::ServicesPage,
    dbus: &VegaDbus,
    action: ServiceAction,
) {
    let page = page.clone();
    let dbus = dbus.clone();
    button.connect_clicked(move |_| {
        let Some(service) = page.selected() else {
            return;
        };
        let (verb, detail) = match action {
            ServiceAction::Enable if service.enabled => (
                gettext("Desabilitar"),
                gettext("deixará de iniciar automaticamente e será parado"),
            ),
            ServiceAction::Enable => (
                gettext("Habilitar"),
                gettext("iniciará agora e automaticamente com o sistema"),
            ),
            ServiceAction::Running if service.active => (
                gettext("Parar"),
                gettext("será interrompido até uma nova inicialização"),
            ),
            ServiceAction::Running => (gettext("Iniciar"), gettext("será iniciado nesta sessão")),
            ServiceAction::Restart => (
                gettext("Reiniciar"),
                gettext("será interrompido e iniciado novamente"),
            ),
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("{verb} serviço?").replace("{verb}", &verb)),
            Some(
                &gettext("{label} ({name}) {detail}.")
                    .replace("{label}", &service.label)
                    .replace("{name}", &service.name)
                    .replace("{detail}", &detail),
            ),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &verb)]);
        if matches!(action, ServiceAction::Restart)
            || matches!(action, ServiceAction::Enable) && !service.enabled
            || matches!(action, ServiceAction::Running) && !service.active
        {
            dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
        }
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = page.clone();
        let dbus = dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.set_busy(true);
            page.status.set_label(
                &gettext("{verb} {label}…")
                    .replace("{verb}", &verb)
                    .replace("{label}", &service.label),
            );
            let result = match action {
                ServiceAction::Enable => {
                    dbus.services()
                        .set_enabled(&service.name, !service.enabled)
                        .await
                }
                ServiceAction::Running => {
                    dbus.services()
                        .set_running(&service.name, !service.active)
                        .await
                }
                ServiceAction::Restart => dbus.services().restart(&service.name).await,
            };
            match result {
                Ok(()) => refresh_services_page(&page, &dbus, page.all.is_active()).await,
                Err(error) => {
                    page.status.set_label(&error.to_string());
                    page.set_busy(false);
                }
            }
        });
    });
}

async fn refresh_services_page(page: &crate::ui::ServicesPage, dbus: &VegaDbus, all: bool) {
    page.status.set_label(&gettext("Carregando serviços…"));
    let result = if all {
        dbus.services().list_all().await
    } else {
        dbus.services().list().await
    };
    match result {
        Ok(items) => page.show(items),
        Err(error) => page.status.set_label(&error.to_string()),
    }
}

// FileChooserNative foi descontinuado na GTK 4.10 em favor de FileDialog —
// migrar é uma tarefa separada (mudança de API, não de acessibilidade);
// por ora só suprime o aviso pra não quebrar `cargo clippy -D warnings`
// depois de habilitar a feature v4_10 (necessária pra gtk::accessible).
#[allow(deprecated)]
fn configure_bluetooth(shell: &VegaShell, window: &adw::ApplicationWindow, dbus: VegaDbus) {
    let page = shell.bluetooth.clone();
    let load_page = page.clone();
    let load_dbus = dbus.clone();
    glib::MainContext::default().spawn_local(async move {
        refresh_bluetooth_page(&load_page, &load_dbus).await;
    });

    let power_page = page.clone();
    let power_dbus = dbus.clone();
    let power_button = page.power.clone();
    power_button.connect_clicked(move |_| {
        let Some(status) = power_page.current_status() else {
            return;
        };
        let enable = !status.powered;
        let verb = if enable {
            gettext("Ligar")
        } else {
            gettext("Desligar")
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("{verb} Bluetooth?").replace("{verb}", &verb)),
            Some(&if enable {
                gettext("O adaptador Bluetooth será ligado.")
            } else {
                gettext("Dispositivos Bluetooth conectados serão desconectados.")
            }),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &verb)]);
        dialog.set_response_appearance(
            "confirm",
            if enable {
                adw::ResponseAppearance::Suggested
            } else {
                adw::ResponseAppearance::Default
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = power_page.clone();
        let dbus = power_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.power.set_sensitive(false);
            match dbus.bluetooth().set_powered(enable).await {
                Ok(()) => refresh_bluetooth_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });

    let scan_page = page.clone();
    let scan_dbus = dbus.clone();
    let scan_button = page.scan.clone();
    scan_button.connect_clicked(move |_| {
        let Some(status) = scan_page.current_status() else {
            return;
        };
        let scanning = !status.scanning;
        let page = scan_page.clone();
        let dbus = scan_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            page.scan.set_sensitive(false);
            page.status.set_label(&if scanning {
                gettext("Iniciando busca Bluetooth…")
            } else {
                gettext("Parando busca Bluetooth…")
            });
            match dbus.bluetooth().set_scanning(scanning).await {
                Ok(()) => refresh_bluetooth_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });

    let device_page = page.clone();
    let device_dbus = dbus.clone();
    let device_button = page.device_action.clone();
    device_button.connect_clicked(move |_| {
        let Some(device) = device_page.selected_device() else {
            return;
        };
        let action = if !device.paired {
            gettext("Parear")
        } else if device.connected {
            gettext("Desconectar")
        } else {
            gettext("Conectar")
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("{action} dispositivo?").replace("{action}", &action)),
            Some(&format!("{} • {}", device.display_name(), device.address)),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &action)]);
        dialog.set_response_appearance(
            "confirm",
            if device.connected {
                adw::ResponseAppearance::Default
            } else {
                adw::ResponseAppearance::Suggested
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = device_page.clone();
        let dbus = device_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.device_action.set_sensitive(false);
            page.status
                .set_label(&gettext("{action} dispositivo…").replace("{action}", &action));
            let result = if !device.paired {
                dbus.bluetooth().pair(&device.address).await
            } else if device.connected {
                dbus.bluetooth().disconnect(&device.address).await
            } else {
                dbus.bluetooth().connect(&device.address).await
            };
            match result {
                Ok(()) => refresh_bluetooth_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });

    let send_page = page.clone();
    let send_dbus = dbus.clone();
    let send_window = window.clone();
    let send_button = page.send_file.clone();
    send_button.connect_clicked(move |_| {
        let Some(device) = send_page.selected_device() else {
            return;
        };
        let chooser = gtk::FileChooserNative::new(
            Some(&gettext("Enviar arquivo por Bluetooth")),
            Some(&send_window),
            gtk::FileChooserAction::Open,
            Some(&gettext("Selecionar")),
            Some(&gettext("Cancelar")),
        );
        let page = send_page.clone();
        let dbus = send_dbus.clone();
        chooser.connect_response(move |chooser, response| {
            chooser.hide();
            if response != gtk::ResponseType::Accept {
                return;
            }
            let Some(path) = chooser.file().and_then(|file| file.path()) else {
                page.status
                    .set_label(&gettext("Selecione um arquivo local."));
                return;
            };
            let display_path = path.display().to_string();
            let dialog = adw::AlertDialog::new(
                Some(&gettext("Enviar arquivo por Bluetooth?")),
                Some(
                    &gettext("Enviar {path} para {device} ({address})?")
                        .replace("{path}", &display_path)
                        .replace("{device}", device.display_name())
                        .replace("{address}", &device.address),
                ),
            );
            dialog.add_responses(&[
                ("cancel", &gettext("Cancelar")),
                ("send", &gettext("Enviar")),
            ]);
            dialog.set_response_appearance("send", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            let page = page.clone();
            let dbus = dbus.clone();
            let address = device.address.clone();
            glib::MainContext::default().spawn_local(async move {
                if !confirm_dialog(&dialog, "send").await {
                    return;
                }
                page.send_file.set_sensitive(false);
                page.status
                    .set_label(&gettext("Enviando arquivo por Bluetooth…"));
                match dbus.bluetooth().send_file(&address, &display_path).await {
                    Ok(()) => page
                        .status
                        .set_label(&gettext("Arquivo enviado com sucesso.")),
                    Err(error) => page.status.set_label(&error.to_string()),
                }
                page.send_file.set_sensitive(true);
            });
        });
        chooser.show();
    });

    let receive_page = page.clone();
    let receive_dbus = dbus;
    let receive_window = window.clone();
    let receive_button = page.receive_files.clone();
    receive_button.connect_clicked(move |_| {
        let chooser = gtk::FileChooserNative::new(
            Some(&gettext("Pasta para arquivos recebidos")),
            Some(&receive_window),
            gtk::FileChooserAction::SelectFolder,
            Some(&gettext("Selecionar")),
            Some(&gettext("Cancelar")),
        );
        let page = receive_page.clone();
        let dbus = receive_dbus.clone();
        chooser.connect_response(move |chooser, response| {
            chooser.hide();
            if response != gtk::ResponseType::Accept {
                return;
            }
            let Some(path) = chooser.file().and_then(|file| file.path()) else {
                page.status.set_label(&gettext("Selecione uma pasta local."));
                return;
            };
            let directory = path.display().to_string();
            let dialog = adw::AlertDialog::new(
                Some(&gettext("Ativar recebimento Bluetooth?")),
                Some(
                    &gettext(
                        "Arquivos recebidos serão gravados em {directory}. Aceite somente transferências esperadas.",
                    )
                    .replace("{directory}", &directory),
                ),
            );
            dialog.add_responses(&[
                ("cancel", &gettext("Cancelar")),
                ("start", &gettext("Ativar")),
            ]);
            dialog.set_response_appearance("start", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            let page = page.clone();
            let dbus = dbus.clone();
            glib::MainContext::default().spawn_local(async move {
                if !confirm_dialog(&dialog, "start").await {
                    return;
                }
                page.receive_files.set_sensitive(false);
                page.status
                    .set_label(&gettext("Ativando recebimento Bluetooth…"));
                match dbus.bluetooth().start_receiver(&directory).await {
                    Ok(()) => refresh_bluetooth_page(&page, &dbus).await,
                    Err(error) => page.status.set_label(&error.to_string()),
                }
            });
        });
        chooser.show();
    });
}

async fn refresh_bluetooth_page(page: &crate::ui::BluetoothPage, dbus: &VegaDbus) {
    let status = dbus.bluetooth().status().await;
    let devices = dbus.bluetooth().devices().await;
    match (status, devices) {
        (Ok(status), Ok(devices)) => page.show(&status, &devices),
        (Err(error), _) | (_, Err(error)) => page.status.set_label(&error.to_string()),
    }
}

// Ver comentário em configure_bluetooth sobre FileChooserNative.
#[allow(deprecated)]
fn configure_network(shell: &VegaShell, window: &adw::ApplicationWindow, dbus: VegaDbus) {
    let page = shell.network.clone();
    let load_page = page.clone();
    let load_dbus = dbus.clone();
    glib::MainContext::default().spawn_local(async move {
        match load_dbus.network().interfaces().await {
            Ok(items) => load_page.show_interfaces(&items),
            Err(error) => {
                load_page.status.set_label(&error.to_string());
                return;
            }
        }
        match load_dbus.network().wifi().await {
            Ok(items) => load_page.show_wifi(&items),
            Err(error) => load_page.status.set_label(&error.to_string()),
        }
        match load_dbus.network().proxy().await {
            Ok(proxy) => load_page.show_proxy(&proxy),
            Err(error) => load_page.proxy.set_label(&error.to_string()),
        }
        refresh_firewall_page(&load_page, &load_dbus).await;
        load_page
            .status
            .set_label(&gettext("Informações de rede carregadas"));
    });

    let interface_page = page.clone();
    let interface_dbus = dbus.clone();
    let interface_action = page.interface_action.clone();
    interface_action.connect_clicked(move |_| {
        let Some(interface) = interface_page.selected_interface() else {
            return;
        };
        let connection = gtk::Entry::builder()
            .text(&interface.name)
            .editable(false)
            .build();
        let address = gtk::Entry::builder()
            .text(&interface.ipv4)
            .placeholder_text("192.168.1.20/24")
            .build();
        let gateway = gtk::Entry::builder()
            .text(&interface.gateway)
            .placeholder_text("192.168.1.1")
            .build();
        let dns = gtk::Entry::builder()
            .text(&interface.dns)
            .placeholder_text("1.1.1.1, 8.8.8.8")
            .build();
        let form = gtk::Grid::builder()
            .column_spacing(10)
            .row_spacing(8)
            .build();
        for (row, (label, field)) in [
            (gettext("Conexão"), connection.clone()),
            (gettext("Endereço/CIDR"), address.clone()),
            ("Gateway".to_string(), gateway.clone()),
            ("DNS".to_string(), dns.clone()),
        ]
        .into_iter()
        .enumerate()
        {
            form.attach(
                &gtk::Label::builder().label(&label).xalign(0.0).build(),
                0,
                row as i32,
                1,
                1,
            );
            field.set_hexpand(true);
            form.attach(&field, 1, row as i32, 1, 1);
        }
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Configurar IPv4 estático?")),
            Some(&gettext(
                "O NetworkManager substituirá a configuração automática e reconectará esta conexão.",
            )),
        );
        dialog.set_extra_child(Some(&form));
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("apply", &gettext("Aplicar")),
        ]);
        dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = interface_page.clone();
        let dbus = interface_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "apply").await {
                return;
            }
            let connection = connection.text().trim().to_owned();
            let address = address.text().trim().to_owned();
            let gateway = gateway.text().trim().to_owned();
            let dns = dns.text().trim().to_owned();
            if connection.is_empty() || !valid_ipv4_cidr(&address) {
                page.status.set_label(&gettext(
                    "Informe uma conexão e um endereço IPv4 com CIDR válido.",
                ));
                return;
            }
            if !gateway.is_empty() && gateway.parse::<std::net::Ipv4Addr>().is_err() {
                page.status
                    .set_label(&gettext("O gateway IPv4 informado é inválido."));
                return;
            }
            page.interface_action.set_sensitive(false);
            page.status.set_label(&gettext("Aplicando IPv4 estático…"));
            match dbus
                .network()
                .set_static_ipv4(&connection, &address, &gateway, &dns)
                .await
            {
                Ok(()) => refresh_interfaces_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });

    let wifi_page = page.clone();
    let wifi_dbus = dbus.clone();
    page.connect_wifi_action(move |network| {
        let disconnect = network.active;
        let password = gtk::PasswordEntry::builder()
            .placeholder_text(gettext("Senha da rede"))
            .show_peek_icon(true)
            .build();
        let dialog = adw::AlertDialog::new(
            Some(&if disconnect {
                gettext("Desconectar do Wi‑Fi?")
            } else {
                gettext("Conectar ao Wi‑Fi?")
            }),
            Some(
                &if disconnect {
                    gettext("{ssid} será desconectada deste dispositivo.")
                } else {
                    gettext("{ssid} será conectada pelo NetworkManager.")
                }
                .replace("{ssid}", &network.ssid),
            ),
        );
        let needs_password = !disconnect && wifi_requires_password(&network.security);
        if needs_password {
            dialog.set_extra_child(Some(&password));
        }
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            (
                "confirm",
                &if disconnect {
                    gettext("Desconectar")
                } else {
                    gettext("Conectar")
                },
            ),
        ]);
        dialog.set_response_appearance(
            "confirm",
            if disconnect {
                adw::ResponseAppearance::Default
            } else {
                adw::ResponseAppearance::Suggested
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = wifi_page.clone();
        let dbus = wifi_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            let secret = password.text().to_string();
            if needs_password && secret.is_empty() {
                page.status
                    .set_label(&gettext("Informe a senha da rede Wi‑Fi."));
                return;
            }
            page.status.set_label(&if disconnect {
                gettext("Desconectando Wi‑Fi…")
            } else {
                gettext("Conectando ao Wi‑Fi…")
            });
            let result = if disconnect {
                dbus.network().disconnect(&network.device).await
            } else {
                dbus.network().connect_wifi(&network.ssid, &secret).await
            };
            match result {
                Ok(()) => refresh_wifi_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });

    let proxy_page = page.clone();
    let proxy_dbus = dbus.clone();
    let proxy_apply = page.proxy_apply.clone();
    proxy_apply.connect_clicked(move |_| {
        let config = proxy_page.proxy_config();
        let clearing = config.http.is_empty()
            && config.https.is_empty()
            && config.socks.is_empty()
            && config.no_proxy.is_empty();
        let dialog = adw::AlertDialog::new(
            Some(&if clearing {
                gettext("Remover configuração de proxy?")
            } else {
                gettext("Aplicar proxy global?")
            }),
            Some(&if clearing {
                gettext("As variáveis de proxy gerenciadas pelo Vega serão removidas de /etc/environment.")
            } else {
                gettext("A configuração será gravada em /etc/environment e poderá exigir uma nova sessão para alcançar todos os aplicativos.")
            }),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            (
                "apply",
                &if clearing {
                    gettext("Remover")
                } else {
                    gettext("Aplicar")
                },
            ),
        ]);
        dialog.set_response_appearance(
            "apply",
            if clearing {
                adw::ResponseAppearance::Destructive
            } else {
                adw::ResponseAppearance::Suggested
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = proxy_page.clone();
        let dbus = proxy_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "apply").await {
                return;
            }
            page.proxy_apply.set_sensitive(false);
            page.proxy
                .set_label(&gettext("Aplicando configuração global…"));
            match dbus.network().set_proxy(&config).await {
                Ok(()) => match dbus.network().proxy().await {
                    Ok(config) => page.show_proxy(&config),
                    Err(error) => page.proxy.set_label(&error.to_string()),
                },
                Err(error) => page.proxy.set_label(&error.to_string()),
            }
            page.proxy_apply.set_sensitive(true);
        });
    });

    let vpn_page = page.clone();
    let vpn_dbus = dbus.clone();
    let vpn_window = window.clone();
    let vpn_import = page.vpn_import.clone();
    vpn_import.connect_clicked(move |_| {
        let chooser = gtk::FileChooserNative::new(
            Some(&gettext("Importar perfil OpenVPN")),
            Some(&vpn_window),
            gtk::FileChooserAction::Open,
            Some(&gettext("Selecionar")),
            Some(&gettext("Cancelar")),
        );
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&gettext("Perfis OpenVPN (*.ovpn)")));
        filter.add_pattern("*.ovpn");
        filter.add_pattern("*.OVPN");
        chooser.add_filter(&filter);
        let page = vpn_page.clone();
        let dbus = vpn_dbus.clone();
        chooser.connect_response(move |chooser, response| {
            chooser.hide();
            if response != gtk::ResponseType::Accept {
                return;
            }
            let Some(path) = chooser.file().and_then(|file| file.path()) else {
                page.vpn_status.set_label(&gettext(
                    "Selecione um arquivo local de perfil OpenVPN.",
                ));
                return;
            };
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ovpn"))
            {
                page.vpn_status
                    .set_label(&gettext("O perfil deve possuir a extensão .ovpn."));
                return;
            }
            let display_path = path.display().to_string();
            let dialog = adw::AlertDialog::new(
                Some(&gettext("Importar perfil OpenVPN?")),
                Some(
                    &gettext(
                        "O NetworkManager importará o perfil:\n{path}\n\nRevise a origem e o conteúdo do arquivo antes de continuar.",
                    )
                    .replace("{path}", &display_path),
                ),
            );
            dialog.add_responses(&[
                ("cancel", &gettext("Cancelar")),
                ("import", &gettext("Importar")),
            ]);
            dialog.set_response_appearance("import", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            let page = page.clone();
            let dbus = dbus.clone();
            glib::MainContext::default().spawn_local(async move {
                if !confirm_dialog(&dialog, "import").await {
                    return;
                }
                page.vpn_import.set_sensitive(false);
                page.vpn_status
                    .set_label(&gettext("Importando perfil OpenVPN…"));
                match dbus.network().import_vpn(&display_path).await {
                    Ok(()) => page
                        .vpn_status
                        .set_label(&gettext("Perfil importado no NetworkManager.")),
                    Err(error) => page.vpn_status.set_label(&error.to_string()),
                }
                page.vpn_import.set_sensitive(true);
            });
        });
        chooser.show();
    });

    let firewall_page = page.clone();
    let firewall_dbus = dbus.clone();
    let firewall_action = page.firewall_action.clone();
    firewall_action.connect_clicked(move |_| {
        let Some(service) = firewall_page.selected_firewall_service() else {
            return;
        };
        let enable = !service.enabled;
        let verb = if enable {
            gettext("Permitir")
        } else {
            gettext("Bloquear")
        };
        let state_word = if enable {
            gettext("permitido")
        } else {
            gettext("bloqueado")
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("{verb} serviço no firewall?").replace("{verb}", &verb)),
            Some(
                &gettext("{label} ({name}) será {state} nas conexões de entrada.")
                    .replace("{label}", &service.label)
                    .replace("{name}", &service.name)
                    .replace("{state}", &state_word),
            ),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &verb)]);
        dialog.set_response_appearance(
            "confirm",
            if enable {
                adw::ResponseAppearance::Suggested
            } else {
                adw::ResponseAppearance::Default
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let action_page = firewall_page.clone();
        let action_dbus = firewall_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            action_page.firewall_action.set_sensitive(false);
            action_page.firewall_status.set_label(
                &gettext("{verb} {label}…")
                    .replace("{verb}", &verb)
                    .replace("{label}", &service.label),
            );
            match action_dbus
                .firewall()
                .set_service(&service.name, enable)
                .await
            {
                Ok(()) => refresh_firewall_page(&action_page, &action_dbus).await,
                Err(error) => action_page.firewall_status.set_label(&error.to_string()),
            }
        });
    });

    let firewall_port_add_page = page.clone();
    let firewall_port_add_dbus = dbus.clone();
    let firewall_port_add = page.firewall_port_add.clone();
    firewall_port_add.connect_clicked(move |_| {
        let (port, protocol) = firewall_port_add_page.firewall_port_input();
        if port.is_empty() {
            firewall_port_add_page
                .firewall_status
                .set_label(&gettext("Informe a porta ou intervalo antes de adicionar."));
            return;
        }
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Adicionar regra de firewall?")),
            Some(
                &gettext("A porta {port}/{protocol} passará a aceitar conexões de entrada.")
                    .replace("{port}", &port)
                    .replace("{protocol}", &protocol),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("confirm", &gettext("Adicionar")),
        ]);
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let action_page = firewall_port_add_page.clone();
        let action_dbus = firewall_port_add_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            action_page.firewall_port_add.set_sensitive(false);
            action_page
                .firewall_status
                .set_label(&gettext("Adicionando regra de firewall…"));
            match action_dbus.firewall().add_port(&port, &protocol).await {
                Ok(()) => {
                    action_page.clear_firewall_port_entry();
                    refresh_firewall_page(&action_page, &action_dbus).await;
                }
                Err(error) => action_page.firewall_status.set_label(&error.to_string()),
            }
            action_page.firewall_port_add.set_sensitive(true);
        });
    });

    let firewall_port_remove = page.firewall_port_remove.clone();
    firewall_port_remove.connect_clicked(move |_| {
        let Some(rule) = page.selected_firewall_port() else {
            return;
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Remover regra de firewall?")),
            Some(
                &gettext("A porta {port}/{protocol} deixará de aceitar conexões de entrada.")
                    .replace("{port}", &rule.port)
                    .replace("{protocol}", &rule.protocol),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("confirm", &gettext("Remover")),
        ]);
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let action_page = page.clone();
        let action_dbus = dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            action_page.firewall_port_remove.set_sensitive(false);
            action_page
                .firewall_status
                .set_label(&gettext("Removendo regra de firewall…"));
            match action_dbus
                .firewall()
                .remove_port(&rule.port, &rule.protocol)
                .await
            {
                Ok(()) => refresh_firewall_page(&action_page, &action_dbus).await,
                Err(error) => {
                    action_page.firewall_status.set_label(&error.to_string());
                    action_page.firewall_port_remove.set_sensitive(true);
                }
            }
        });
    });
}

async fn refresh_wifi_page(page: &crate::ui::NetworkPage, dbus: &VegaDbus) {
    match dbus.network().wifi().await {
        Ok(items) => {
            page.show_wifi(&items);
            page.status.set_label(&gettext("Redes Wi‑Fi atualizadas"));
        }
        Err(error) => page.status.set_label(&error.to_string()),
    }
}

async fn refresh_interfaces_page(page: &crate::ui::NetworkPage, dbus: &VegaDbus) {
    match dbus.network().interfaces().await {
        Ok(items) => {
            page.show_interfaces(&items);
            page.status
                .set_label(&gettext("Interfaces de rede atualizadas"));
        }
        Err(error) => page.status.set_label(&error.to_string()),
    }
}

fn valid_ipv4_cidr(value: &str) -> bool {
    let Some((address, prefix)) = value.split_once('/') else {
        return false;
    };
    address.parse::<std::net::Ipv4Addr>().is_ok()
        && prefix.parse::<u8>().is_ok_and(|prefix| prefix <= 32)
}

fn wifi_requires_password(security: &str) -> bool {
    !matches!(
        security.trim().to_ascii_lowercase().as_str(),
        "" | "--" | "open" | "none"
    )
}

async fn refresh_firewall_page(page: &crate::ui::NetworkPage, dbus: &VegaDbus) {
    let status = dbus.firewall().status().await;
    let services = dbus.firewall().services().await;
    match (status, services) {
        (Ok(status), Ok(services)) => page.show_firewall(&status, &services),
        (Err(error), _) | (_, Err(error)) => {
            page.firewall_status.set_label(&error.to_string());
            page.firewall_action.set_sensitive(false);
        }
    }
    match dbus.firewall().ports().await {
        Ok(ports) => page.show_firewall_ports(&ports),
        Err(error) => page.firewall_status.set_label(&error.to_string()),
    }
}

fn configure_storage(shell: &VegaShell, dbus: VegaDbus) {
    let page = shell.storage.clone();
    let load_page = page.clone();
    let load_dbus = dbus.clone();
    glib::MainContext::default().spawn_local(async move {
        refresh_storage_page(&load_page, &load_dbus).await;
    });
    let action_page = page;
    let action_dbus = dbus;
    shell.storage.action.connect_clicked(move |_| {
        let Some(volume) = action_page.selected() else {
            return;
        };
        let unmount = volume.can_unmount;
        let verb = if unmount {
            gettext("Desmontar")
        } else {
            gettext("Montar")
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("{verb} volume?").replace("{verb}", &verb)),
            Some(&format!(
                "{} ({})",
                volume.path,
                if volume.mountpoint.is_empty() {
                    gettext("não montado")
                } else {
                    volume.mountpoint.clone()
                }
            )),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &verb)]);
        dialog.set_response_appearance(
            "confirm",
            if unmount {
                adw::ResponseAppearance::Default
            } else {
                adw::ResponseAppearance::Suggested
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = action_page.clone();
        let dbus = action_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.action.set_sensitive(false);
            page.status.set_label(
                &gettext("{verb} {path}…")
                    .replace("{verb}", &verb)
                    .replace("{path}", &volume.path),
            );
            let result = if unmount {
                dbus.storage().unmount(&volume.path).await
            } else {
                dbus.storage().mount(&volume.path).await
            };
            match result {
                Ok(()) => refresh_storage_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });
}

async fn refresh_storage_page(page: &crate::ui::StoragePage, dbus: &VegaDbus) {
    match dbus.storage().list().await {
        Ok(volumes) => page.show(volumes),
        Err(error) => page.status.set_label(&error.to_string()),
    }
}

fn configure_screen(shell: &VegaShell, window: &adw::ApplicationWindow, dbus: VegaDbus) {
    configure_wallpaper_tab(&shell.screen.wallpaper, window);
    configure_screensaver_tab(&shell.screen.screensaver);
    configure_menu_tab(&shell.screen.menu);
    configure_dock_tab(&shell.screen.dock);
    configure_monitor_tab(&shell.monitor, dbus);
}

fn configure_wallpaper_tab(page: &crate::ui::WallpaperPage, window: &adw::ApplicationWindow) {
    let load_page = page.clone();
    glib::MainContext::default().spawn_local(async move {
        refresh_wallpaper_page(&load_page).await;
    });

    let apply_page = page.clone();
    page.connect_apply(move |entry| match crate::wallpaper::apply(&entry) {
        Ok(()) => {
            apply_page
                .status
                .set_label(&gettext("{name} aplicado.").replace("{name}", &entry.name));
            let page = apply_page.clone();
            glib::MainContext::default().spawn_local(async move {
                refresh_wallpaper_page(&page).await;
            });
        }
        Err(error) => apply_page.status.set_label(&error.to_string()),
    });

    let add_page = page.clone();
    let add_window = window.clone();
    page.add.connect_clicked(move |button| {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&gettext("Imagens")));
        filter.add_mime_type("image/*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Adicionar wallpaper"))
            .accept_label(gettext("Adicionar"))
            .filters(&filters)
            .default_filter(&filter)
            .modal(true)
            .build();
        let page = add_page.clone();
        let window = add_window.clone();
        let button = button.clone();
        glib::MainContext::default().spawn_local(async move {
            let Ok(file) = dialog.open_future(Some(&window)).await else {
                return;
            };
            let Some(path) = file.path() else {
                page.status
                    .set_label(&gettext("Selecione um arquivo de imagem local."));
                return;
            };

            button.set_sensitive(false);
            page.status.set_label(&gettext("Adicionando wallpaper…"));
            let result = gio::spawn_blocking(move || crate::wallpaper::import(&path)).await;
            match result {
                Ok(Ok(entry)) => {
                    let apply_result = crate::wallpaper::apply(&entry);
                    refresh_wallpaper_page(&page).await;
                    match apply_result {
                        Ok(()) => page.status.set_label(
                            &gettext("{name} adicionado e aplicado.")
                                .replace("{name}", &entry.name),
                        ),
                        Err(error) => page.status.set_label(&error.to_string()),
                    }
                }
                Ok(Err(error)) => page.status.set_label(&error.to_string()),
                Err(_) => page
                    .status
                    .set_label(&gettext("Falha interna ao adicionar o wallpaper.")),
            }
            button.set_sensitive(true);
        });
    });
}

async fn refresh_wallpaper_page(page: &crate::ui::WallpaperPage) {
    // Usa a aparência efetiva da sessão (inclusive quando o esquema está em
    // "seguir o sistema") para que o card não mostre sempre a variante clara.
    let prefer_dark = adw::StyleManager::default().is_dark();
    let result = gio::spawn_blocking(move || {
        let wallpapers = crate::wallpaper::list_wallpapers();
        let thumbnails = crate::wallpaper::load_thumbnails(&wallpapers, prefer_dark);
        (wallpapers, thumbnails)
    })
    .await;
    match result {
        Ok((wallpapers, thumbnails)) => {
            let current = crate::wallpaper::current_light_path();
            page.show(&wallpapers, &thumbnails, current.as_deref());
        }
        Err(_) => page
            .status
            .set_label(&gettext("Falha interna ao carregar os papéis de parede.")),
    }
}

fn configure_screensaver_tab(page: &crate::ui::ScreensaverPage) {
    match crate::screensaver::current() {
        Some(settings) => page.show(&settings),
        None => page.status.set_label(&gettext(
            "Bloqueio de tela não está disponível neste sistema.",
        )),
    }

    let apply_page = page.clone();
    page.apply.connect_clicked(move |_| {
        let settings = apply_page.selected();
        match crate::screensaver::apply(&settings) {
            Ok(()) => apply_page
                .status
                .set_label(&gettext("Configuração aplicada.")),
            Err(error) => apply_page.status.set_label(&error.to_string()),
        }
    });
}

fn configure_dock_tab(page: &crate::ui::DockPage) {
    match crate::dock::current() {
        Some(settings) => page.show(&settings),
        None => page
            .status
            .set_label(&gettext("A extensão Sheliak não está instalada.")),
    }

    let apply_page = page.clone();
    page.connect_changed(move |settings| match crate::dock::apply(&settings) {
        Ok(()) => apply_page
            .status
            .set_label(&gettext("Configuração aplicada.")),
        Err(error) => apply_page.status.set_label(&error.to_string()),
    });
}

fn configure_menu_tab(page: &crate::ui::MenuPage) {
    match crate::dock::current_menu() {
        Some(settings) => page.show(&settings),
        None => page
            .status
            .set_label(&gettext("A extensão Sheliak não está instalada.")),
    }

    let apply_page = page.clone();
    page.connect_changed(move |settings| match crate::dock::apply_menu(&settings) {
        Ok(()) => apply_page
            .status
            .set_label(&gettext("Configuração aplicada.")),
        Err(error) => apply_page.status.set_label(&error.to_string()),
    });
}

fn configure_monitor_tab(page: &crate::ui::MonitorPage, dbus: VegaDbus) {
    let last_metrics: Rc<RefCell<Option<(lyra_vega_dbus::SystemMetrics, std::time::Instant)>>> =
        Rc::new(RefCell::new(None));
    let timer_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    // O timer só roda enquanto a aba está visível (map/unmap do próprio
    // widget) — atende ao requisito original da issue de monitor de não
    // gastar CPU sondando processos com a página fora de vista.
    let map_page = page.clone();
    let map_dbus = dbus.clone();
    let map_last = last_metrics.clone();
    let map_timer = timer_id.clone();
    page.root.connect_map(move |_| {
        if map_timer.borrow().is_some() {
            return;
        }
        let tick_page = map_page.clone();
        let tick_dbus = map_dbus.clone();
        let tick_last = map_last.clone();
        glib::MainContext::default().spawn_local({
            let page = tick_page.clone();
            let dbus = tick_dbus.clone();
            let last = tick_last.clone();
            async move {
                refresh_monitor_page(&page, &dbus, &last).await;
            }
        });
        let id = glib::timeout_add_seconds_local(2, move || {
            let page = tick_page.clone();
            let dbus = tick_dbus.clone();
            let last = tick_last.clone();
            glib::MainContext::default().spawn_local(async move {
                refresh_monitor_page(&page, &dbus, &last).await;
            });
            glib::ControlFlow::Continue
        });
        *map_timer.borrow_mut() = Some(id);
    });

    let unmap_timer = timer_id.clone();
    page.root.connect_unmap(move |_| {
        if let Some(id) = unmap_timer.borrow_mut().take() {
            id.remove();
        }
    });

    let kill_page = page.clone();
    page.connect_kill(move |process| {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Encerrar processo?")),
            Some(
                &gettext("{name} (PID {pid}) será encerrado com SIGTERM.")
                    .replace("{name}", &process.name)
                    .replace("{pid}", &process.pid.to_string()),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("confirm", &gettext("Encerrar")),
        ]);
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = kill_page.clone();
        let dbus = dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            match dbus.monitor().kill_process(process.pid).await {
                Ok(()) => page
                    .status
                    .set_label(&gettext("Sinal enviado ao processo.")),
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });
}

async fn refresh_monitor_page(
    page: &crate::ui::MonitorPage,
    dbus: &VegaDbus,
    last: &Rc<RefCell<Option<(lyra_vega_dbus::SystemMetrics, std::time::Instant)>>>,
) {
    let client = dbus.monitor();
    match client.metrics().await {
        Ok(metrics) => {
            let now = std::time::Instant::now();
            let rates = last.borrow().as_ref().map(|(previous, previous_time)| {
                let elapsed = now.duration_since(*previous_time).as_secs_f64().max(0.001);
                crate::ui::Rates {
                    disk_read_per_sec: rate_per_sec(
                        previous.disk_read_bytes,
                        metrics.disk_read_bytes,
                        elapsed,
                    ),
                    disk_write_per_sec: rate_per_sec(
                        previous.disk_write_bytes,
                        metrics.disk_write_bytes,
                        elapsed,
                    ),
                    net_rx_per_sec: rate_per_sec(
                        previous.net_rx_bytes,
                        metrics.net_rx_bytes,
                        elapsed,
                    ),
                    net_tx_per_sec: rate_per_sec(
                        previous.net_tx_bytes,
                        metrics.net_tx_bytes,
                        elapsed,
                    ),
                }
            });
            page.show_metrics(&metrics, rates);
            *last.borrow_mut() = Some((metrics, now));
        }
        Err(error) => page.status.set_label(&error.to_string()),
    }
    match client.list_processes().await {
        Ok(processes) => page.show_processes(processes),
        Err(error) => page.processes_status.set_label(&error.to_string()),
    }
}

fn rate_per_sec(previous: u64, current: u64, elapsed_secs: f64) -> u64 {
    let delta = current.saturating_sub(previous);
    (delta as f64 / elapsed_secs) as u64
}

fn configure_datetime(shell: &VegaShell, dbus: VegaDbus) {
    let page = shell.datetime.clone();
    let load_page = page.clone();
    let load_dbus = dbus.clone();
    glib::MainContext::default().spawn_local(async move {
        refresh_datetime_page(&load_page, &load_dbus).await;
    });

    let apply_page = page;
    let apply_dbus = dbus;
    shell.datetime.apply.connect_clicked(move |_| {
        let timezone = crate::ui::DateTimePage::selected(&apply_page.timezone);
        let locale = crate::ui::DateTimePage::selected(&apply_page.locale);
        let keymap = crate::ui::DateTimePage::selected(&apply_page.keymap);
        let ntp = apply_page.ntp.is_active();
        let ntp_word = if ntp {
            gettext("ativado")
        } else {
            gettext("desativado")
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Alterar data, hora e idioma?")),
            Some(
                &gettext(
                    "Aplicar timezone {timezone}, locale {locale}, teclado {keymap} e NTP {ntp} para todo o sistema?",
                )
                .replace("{timezone}", &timezone)
                .replace("{locale}", &locale)
                .replace("{keymap}", &keymap)
                .replace("{ntp}", &ntp_word),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("apply", &gettext("Aplicar")),
        ]);
        dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = apply_page.clone();
        let dbus = apply_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "apply").await {
                return;
            }
            page.apply.set_sensitive(false);
            page.status
                .set_label(&gettext("Aplicando configuração global…"));
            match dbus
                .datetime()
                .apply(&timezone, ntp, &locale, &keymap)
                .await
            {
                Ok(()) => refresh_datetime_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
            page.apply.set_sensitive(true);
        });
    });
}

async fn refresh_datetime_page(page: &crate::ui::DateTimePage, dbus: &VegaDbus) {
    let status = dbus.datetime().status().await;
    let timezones = dbus.datetime().list_timezones().await;
    let locales = dbus.datetime().list_locales().await;
    let keymaps = dbus.datetime().list_keymaps().await;
    match (status, timezones, locales, keymaps) {
        (Ok(status), Ok(timezones), Ok(locales), Ok(keymaps)) => {
            page.show(&status, &timezones, &locales, &keymaps);
        }
        (Err(error), _, _, _)
        | (_, Err(error), _, _)
        | (_, _, Err(error), _)
        | (_, _, _, Err(error)) => page.status.set_label(&error.to_string()),
    }
}

fn configure_kernel(shell: &VegaShell, dbus: VegaDbus) {
    let page = shell.kernel.clone();
    let load_dbus = dbus.clone();
    let load_page = page.clone();
    glib::MainContext::default().spawn_local(async move {
        refresh_kernel_page(&load_page, &load_dbus).await;
    });

    let boot_page = page;
    let boot_dbus = dbus;
    shell.kernel.apply_boot.connect_clicked(move |_| {
        let default_entry = boot_page.selected_boot_entry();
        let timeout = boot_page.boot_timeout_input.value_as_int().max(0) as u32;
        let cmdline = boot_page.boot_cmdline_input.text().trim().to_owned();
        let entry_label = if default_entry.is_empty() {
            gettext("padrão atual")
        } else {
            default_entry.clone()
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Alterar configuração de boot?")),
            Some(
                &gettext(
                    "Aplicar entrada padrão '{entry}', timeout de {timeout} segundo(s) e os parâmetros informados? Um snapshot será criado quando possível.",
                )
                .replace("{entry}", &entry_label)
                .replace("{timeout}", &timeout.to_string()),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("apply", &gettext("Aplicar")),
        ]);
        dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = boot_page.clone();
        let dbus = boot_dbus.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "apply").await {
                return;
            }
            page.apply_boot.set_sensitive(false);
            page.status
                .set_label(&gettext("Aplicando configuração de boot…"));
            match dbus
                .kernel()
                .apply_boot_config(&default_entry, timeout, &cmdline)
                .await
            {
                Ok(()) => refresh_kernel_page(&page, &dbus).await,
                Err(error) => page.status.set_label(&error.to_string()),
            }
            page.apply_boot.set_sensitive(true);
        });
    });
}

async fn refresh_kernel_page(page: &crate::ui::KernelPage, dbus: &VegaDbus) {
    let installed = match dbus.kernel().list_installed().await {
        Ok(kernels) => {
            page.show_installed(&kernels);
            kernels
        }
        Err(error) => {
            page.status.set_label(&error.to_string());
            return;
        }
    };
    match dbus.kernel().available_packages().await {
        Ok(kernels) => page.show_available(&kernels, &installed),
        Err(error) => page.status.set_label(&error.to_string()),
    }
    let boot = dbus.kernel().boot_status().await;
    let entries = dbus.kernel().list_boot_entries().await;
    match (boot, entries) {
        (Ok(boot), Ok(entries)) => {
            page.show_boot(&boot, &entries);
            page.status
                .set_label(&gettext("Informações carregadas pelo vegad"));
        }
        (Err(error), _) | (_, Err(error)) => page.status.set_label(&error.to_string()),
    }
}

fn configure_snapshots(shell: &VegaShell, dbus: VegaDbus) {
    let page = shell.snapshots.clone();
    let client = dbus.snapshots();
    let load_page = page.clone();
    glib::MainContext::default().spawn_local(async move {
        match client.available().await {
            Ok(false) => load_page.set_available(false),
            Ok(true) => match client.list().await {
                Ok(snapshots) => load_page.show_snapshots(snapshots),
                Err(error) => load_page.status.set_label(&error.to_string()),
            },
            Err(error) => {
                load_page.set_available(false);
                load_page.status.set_label(&error.to_string());
            }
        }
    });

    let compare_page = page;
    let compare_dbus = dbus.clone();
    shell.snapshots.connect_compare(move |snapshot, _button| {
        compare_page
            .comparison
            .set_label(&gettext("Comparando pacotes…"));
        let page = compare_page.clone();
        let client = compare_dbus.snapshots();
        glib::MainContext::default().spawn_local(async move {
            match client.diff_packages(snapshot.id).await {
                Ok(changes) => page.show_comparison(&changes),
                Err(error) => page.comparison.set_label(&error.to_string()),
            }
        });
    });

    let create_page = shell.snapshots.clone();
    let create_dbus = dbus.clone();
    shell.snapshots.create.connect_clicked(move |_| {
        let description = gtk::Entry::builder()
            .placeholder_text(gettext("Descrição do ponto de restauração"))
            .build();
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Criar ponto de restauração?")),
            Some(&gettext(
                "O snapshot será criado pelo backend disponível no sistema.",
            )),
        );
        dialog.set_extra_child(Some(&description));
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("create", &gettext("Criar")),
        ]);
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = create_page.clone();
        let client = create_dbus.snapshots();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "create").await {
                return;
            }
            let description = description.text().trim().to_owned();
            if description.is_empty() {
                page.status
                    .set_label(&gettext("Informe uma descrição para o snapshot."));
                return;
            }
            page.create.set_sensitive(false);
            page.status
                .set_label(&gettext("Criando ponto de restauração…"));
            match client.create(&description).await {
                Ok(_) => match client.list().await {
                    Ok(snapshots) => page.show_snapshots(snapshots),
                    Err(error) => page.status.set_label(&error.to_string()),
                },
                Err(error) => page.status.set_label(&error.to_string()),
            }
            page.create.set_sensitive(true);
        });
    });

    let delete_page = shell.snapshots.clone();
    let delete_dbus = dbus.clone();
    shell.snapshots.connect_delete(move |snapshot, button| {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Excluir ponto de restauração?")),
            Some(
                &gettext("O snapshot #{id} será excluído permanentemente.")
                    .replace("{id}", &snapshot.id.to_string()),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("delete", &gettext("Excluir")),
        ]);
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = delete_page.clone();
        let client = delete_dbus.snapshots();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "delete").await {
                return;
            }
            button.set_sensitive(false);
            page.status
                .set_label(&gettext("Excluindo ponto de restauração…"));
            match client.delete(snapshot.id).await {
                Ok(()) => match client.list().await {
                    Ok(snapshots) => page.show_snapshots(snapshots),
                    Err(error) => page.status.set_label(&error.to_string()),
                },
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });

    let clear_page = shell.snapshots.clone();
    let clear_dbus = dbus.clone();
    shell.snapshots.clear.connect_clicked(move |button| {
        let page = clear_page.clone();
        let client = clear_dbus.snapshots();
        let button = button.clone();
        glib::MainContext::default().spawn_local(async move {
            let snapshots = match client.list().await {
                Ok(snapshots) => snapshots,
                Err(error) => {
                    page.status.set_label(&error.to_string());
                    return;
                }
            };
            let deletable = snapshots
                .into_iter()
                .filter(|snapshot| snapshot.id != 0)
                .collect::<Vec<_>>();
            if deletable.is_empty() {
                page.status
                    .set_label(&gettext("Não há pontos de restauração que possam ser excluídos."));
                button.set_sensitive(false);
                return;
            }

            let dialog = adw::AlertDialog::new(
                Some(&gettext("Limpar pontos de restauração criados?")),
                Some(
                    &gettext(
                        "Os {count} pontos que podem ser removidos serão excluídos permanentemente. O snapshot #0, quando presente, será preservado.",
                    )
                    .replace("{count}", &deletable.len().to_string()),
                ),
            );
            dialog.add_responses(&[
                ("cancel", &gettext("Cancelar")),
                ("clear", &gettext("Limpar pontos")),
            ]);
            dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            if !confirm_dialog(&dialog, "clear").await {
                return;
            }

            button.set_sensitive(false);
            page.status
                .set_label(&gettext("Limpando pontos de restauração…"));
            page.clear_progress.set_visible(true);
            let progress = page.clear_progress.clone();
            progress.set_fraction(0.0);
            progress.set_text(Some(
                &gettext("Excluindo 0 de {total} pontos…")
                    .replace("{total}", &deletable.len().to_string()),
            ));
            let result = client
                .clear_with_progress(move |deleted, total| {
                    progress.set_fraction(deleted as f64 / total.max(1) as f64);
                    progress.set_text(Some(
                        &gettext("Excluindo {deleted} de {total} pontos…")
                            .replace("{deleted}", &deleted.to_string())
                            .replace("{total}", &total.to_string()),
                    ));
                })
                .await;
            page.clear_progress.set_visible(false);

            match client.list().await {
                Ok(snapshots) => page.show_snapshots(snapshots),
                Err(error) => {
                    page.status.set_label(&error.to_string());
                    return;
                }
            }
            match result {
                Ok(deleted) => page.status.set_label(
                    &gettext("{count} pontos de restauração foram excluídos.")
                        .replace("{count}", &deleted.to_string()),
                ),
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });

    let rollback_page = shell.snapshots.clone();
    let rollback_dbus = dbus.clone();
    shell.snapshots.connect_apply(move |snapshot, button| {
        button.set_sensitive(false);
        rollback_page
            .comparison
            .set_label(&gettext("Carregando revisão obrigatória do rollback…"));
        let page = rollback_page.clone();
        let client = rollback_dbus.snapshots();
        glib::MainContext::default().spawn_local(async move {
            let changes = match client.diff_packages(snapshot.id).await {
                Ok(changes) => changes,
                Err(error) => {
                    page.comparison.set_label(&error.to_string());
                    button.set_sensitive(true);
                    return;
                }
            };
            page.show_comparison(&changes);
            let preview = gtk::TextView::builder()
                .editable(false)
                .cursor_visible(false)
                .monospace(true)
                .wrap_mode(gtk::WrapMode::WordChar)
                .build();
            let preview_text = if changes.is_empty() {
                gettext(
                    "Nenhuma diferença de pacotes foi detectada. Arquivos do sistema ainda podem ser alterados.",
                )
            } else {
                changes.join("\n")
            };
            preview.buffer().set_text(&preview_text);
            let scroll = gtk::ScrolledWindow::builder()
                .child(&preview)
                .min_content_width(560)
                .min_content_height(240)
                .max_content_height(400)
                .build();
            let dialog = adw::AlertDialog::new(
                Some(
                    &gettext("Aplicar snapshot #{id}?").replace("{id}", &snapshot.id.to_string()),
                ),
                Some(&gettext(
                    "Revise as diferenças. O sistema poderá precisar ser reiniciado após o rollback.",
                )),
            );
            dialog.set_extra_child(Some(&scroll));
            dialog.add_responses(&[
                ("cancel", &gettext("Cancelar")),
                ("rollback", &gettext("Aplicar rollback")),
            ]);
            dialog.set_response_appearance("rollback", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            if !confirm_dialog(&dialog, "rollback").await {
                button.set_sensitive(true);
                return;
            }
            page.status
                .set_label(&gettext("Aplicando ponto de restauração…"));
            match client.rollback(snapshot.id).await {
                Ok(()) => page.status.set_label(&gettext(
                    "Rollback aplicado. Reinicie o sistema se o backend solicitar.",
                )),
                Err(error) => page.status.set_label(&error.to_string()),
            }
            button.set_sensitive(true);
        });
    });

    let retention_page = shell.snapshots.clone();
    let retention_dbus = dbus;
    shell.snapshots.apply_retention.connect_clicked(move |_| {
        let keep = retention_page.retention.value_as_int().max(1) as u32;
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Alterar política de retenção?")),
            Some(
                &gettext(
                    "O sistema manterá os {keep} snapshots mais recentes e removerá imediatamente os mais antigos.",
                )
                .replace("{keep}", &keep.to_string()),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("apply", &gettext("Aplicar")),
        ]);
        dialog.set_response_appearance("apply", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = retention_page.clone();
        let client = retention_dbus.snapshots();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "apply").await {
                return;
            }
            page.apply_retention.set_sensitive(false);
            match client.set_retention(keep).await {
                Ok(()) => match client.list().await {
                    Ok(snapshots) => {
                        page.show_snapshots(snapshots);
                        page.status.set_label(
                            &gettext("Retenção aplicada: mantendo os {keep} snapshots mais recentes.")
                                .replace("{keep}", &keep.to_string()),
                        );
                    }
                    Err(error) => page.status.set_label(&error.to_string()),
                },
                Err(error) => page.status.set_label(&error.to_string()),
            }
            page.apply_retention.set_sensitive(true);
        });
    });
}

fn configure_backup(shell: &VegaShell, dbus: VegaDbus) {
    let page = shell.backup.clone();
    let client = dbus.backup();
    let load_page = page.clone();
    glib::MainContext::default().spawn_local(async move {
        match client.list_configs().await {
            Ok(configs) => load_page.show_configs(configs),
            Err(error) => load_page.status.set_label(&error.to_string()),
        }
    });

    let snapshots_page = page.clone();
    let snapshots_dbus = dbus.clone();
    page.configs.connect_row_selected(move |_, row| {
        if row.is_none() {
            return;
        }
        let Some(config) = snapshots_page.selected_config() else {
            return;
        };
        snapshots_page
            .snapshot_status
            .set_label(&gettext("Carregando snapshots…"));
        let page = snapshots_page.clone();
        let client = snapshots_dbus.backup();
        glib::MainContext::default().spawn_local(async move {
            match client.list_snapshots(&config.id).await {
                Ok(snapshots) => page.show_snapshots(snapshots),
                Err(error) => page.snapshot_status.set_label(&error.to_string()),
            }
        });
    });

    let paths_page = page.clone();
    let paths_dbus = dbus.clone();
    page.snapshots.connect_row_selected(move |_, row| {
        if row.is_none() {
            return;
        }
        let (Some(config), Some(snapshot)) =
            (paths_page.selected_config(), paths_page.selected_snapshot())
        else {
            return;
        };
        paths_page
            .snapshot_paths
            .set_label(&gettext("Carregando caminhos…"));
        let page = paths_page.clone();
        let client = paths_dbus.backup();
        glib::MainContext::default().spawn_local(async move {
            match client.list_snapshot_paths(&config.id, &snapshot.id).await {
                Ok(paths) => page.show_snapshot_paths(&paths),
                Err(error) => page.snapshot_paths.set_label(&error.to_string()),
            }
        });
    });

    let create_page = page.clone();
    let create_dbus = dbus.clone();
    page.new_config.connect_clicked(move |_| {
        let id = gtk::Entry::builder()
            .placeholder_text(gettext("Identificador, por exemplo documentos"))
            .build();
        let paths = gtk::Entry::builder()
            .placeholder_text(gettext("Caminhos separados por vírgula"))
            .build();
        let destination = gtk::Entry::builder()
            .placeholder_text(gettext("Diretório ou repositório de destino"))
            .build();
        let destination_uuid = gtk::Entry::builder()
            .placeholder_text(gettext("UUID do volume (opcional)"))
            .build();
        let frequency = gtk::DropDown::from_strings(&[
            &gettext("Manual"),
            &gettext("Diário"),
            &gettext("Semanal"),
        ]);
        let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
        form.add_css_class("backup-config-form");
        for (label, field) in [
            (gettext("Identificador"), id.clone().upcast::<gtk::Widget>()),
            (gettext("Caminhos"), paths.clone().upcast()),
            (gettext("Destino"), destination.clone().upcast()),
            (
                gettext("UUID do destino"),
                destination_uuid.clone().upcast(),
            ),
            (gettext("Frequência"), frequency.clone().upcast()),
        ] {
            form.append(
                &gtk::Label::builder()
                    .label(&label)
                    .xalign(0.0)
                    .css_classes(["dim-label"])
                    .build(),
            );
            form.append(&field);
        }
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Nova configuração de backup")),
            Some(&gettext(
                "Os caminhos serão validados pelo vegad antes de serem salvos.",
            )),
        );
        dialog.set_extra_child(Some(&form));
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("create", &gettext("Criar")),
        ]);
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = create_page.clone();
        let client = create_dbus.backup();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "create").await {
                return;
            }
            let parsed_paths = paths
                .text()
                .split(',')
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            let id = id.text().trim().to_owned();
            let destination = destination.text().trim().to_owned();
            if id.is_empty() || destination.is_empty() || parsed_paths.is_empty() {
                page.status.set_label(&gettext(
                    "Identificador, ao menos um caminho e destino são obrigatórios.",
                ));
                return;
            }
            let frequency = match frequency.selected() {
                1 => "daily",
                2 => "weekly",
                _ => "manual",
            };
            page.new_config.set_sensitive(false);
            page.status.set_label(&gettext("Criando configuração…"));
            let config = BackupConfig {
                id,
                paths: parsed_paths,
                destination,
                destination_uuid: destination_uuid.text().trim().to_owned(),
                frequency: frequency.into(),
            };
            match client.create_config(config).await {
                Ok(_) => match client.list_configs().await {
                    Ok(configs) => page.show_configs(configs),
                    Err(error) => page.status.set_label(&error.to_string()),
                },
                Err(error) => page.status.set_label(&error.to_string()),
            }
            page.new_config.set_sensitive(true);
        });
    });

    let delete_page = page.clone();
    let delete_dbus = dbus.clone();
    page.delete_config.connect_clicked(move |_| {
        let Some(config) = delete_page.selected_config() else {
            return;
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Excluir configuração de backup?")),
            Some(
                &gettext(
                    "A configuração {id} será removida. Os dados já armazenados no destino não serão apagados automaticamente.",
                )
                .replace("{id}", &config.id),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("delete", &gettext("Excluir")),
        ]);
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = delete_page.clone();
        let client = delete_dbus.backup();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "delete").await {
                return;
            }
            page.delete_config.set_sensitive(false);
            page.status.set_label(&gettext("Excluindo configuração…"));
            match client.delete_config(&config.id).await {
                Ok(()) => match client.list_configs().await {
                    Ok(configs) => page.show_configs(configs),
                    Err(error) => page.status.set_label(&error.to_string()),
                },
                Err(error) => page.status.set_label(&error.to_string()),
            }
        });
    });

    let restore_page = page.clone();
    let restore_dbus = dbus.clone();
    page.restore_selected.connect_clicked(move |_| {
        let Some(snapshot) = restore_page.selected_snapshot() else {
            return;
        };
        let paths = restore_page.selected_paths();
        let target = restore_page.restore_target.text().trim().to_owned();
        if target.is_empty() {
            restore_page
                .snapshot_paths
                .set_label(&gettext("Informe a pasta de destino antes de restaurar."));
            return;
        }
        if paths.is_empty() {
            restore_page.snapshot_paths.set_label(&gettext(
                "Selecione ao menos um caminho para restaurar.",
            ));
            return;
        }
        let mode = restore_page.restore_mode_value();
        let replacing = mode == "replace";
        let body = if replacing {
            gettext(
                "{count} item(ns) poderão substituir arquivos existentes em {target}. Esta ação pode causar perda de dados.",
            )
        } else {
            gettext(
                "{count} item(ns) serão restaurados em uma pasta separada dentro de {target}.",
            )
        }
        .replace("{count}", &paths.len().to_string())
        .replace("{target}", &target);
        let dialog = adw::AlertDialog::new(
            Some(&if replacing {
                gettext("Substituir arquivos existentes?")
            } else {
                gettext("Restaurar em pasta separada?")
            }),
            Some(&body),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("confirm", &gettext("Restaurar")),
        ]);
        dialog.set_response_appearance(
            "confirm",
            if replacing {
                adw::ResponseAppearance::Destructive
            } else {
                adw::ResponseAppearance::Suggested
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = restore_page.clone();
        let client = restore_dbus.backup();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.restore_selected.set_sensitive(false);
            page.begin(&gettext("Preparando restauração parcial…"));
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => {
                    page.finish(false, &error.to_string());
                    page.restore_selected.set_sensitive(true);
                    return;
                }
            };
            match client
                .restore_items(&snapshot.id, &target, mode, &paths)
                .await
            {
                Ok(id) => monitor_restore_transaction(&page, &mut events, id).await,
                Err(error) => page.finish(false, &error.to_string()),
            }
            page.restore_selected.set_sensitive(true);
        });
    });

    let run_page = page;
    let run_dbus = dbus;
    shell.backup.run_now.connect_clicked(move |_| {
        let Some(config) = run_page.selected_config() else {
            return;
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Executar backup agora?")),
            Some(
                &gettext("Configuração {id} para {count} caminho(s).")
                    .replace("{id}", &config.id)
                    .replace("{count}", &config.paths.len().to_string()),
            ),
        );
        dialog.add_responses(&[
            ("cancel", &gettext("Cancelar")),
            ("confirm", &gettext("Executar")),
        ]);
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = run_page.clone();
        let client = run_dbus.backup();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.run_now.set_sensitive(false);
            page.begin(&gettext("Iniciando backup {id}…").replace("{id}", &config.id));
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => {
                    page.finish(false, &error.to_string());
                    page.run_now.set_sensitive(true);
                    return;
                }
            };
            match client.run_now(&config.id).await {
                Ok(id) => monitor_backup_transaction(&page, &mut events, id).await,
                Err(error) => page.finish(false, &error.to_string()),
            }
            page.run_now.set_sensitive(true);
        });
    });
}

async fn monitor_restore_transaction(
    page: &crate::ui::BackupPage,
    events: &mut lyra_vega_dbus::BackupEventStream,
    transaction_id: u32,
) {
    loop {
        match events.next().await {
            Ok(BackupEvent::RestoreProgress(progress))
                if progress.transaction_id == transaction_id =>
            {
                page.update_progress(progress.percent, &progress.message);
            }
            Ok(BackupEvent::RestoreFinished(finished))
                if finished.transaction_id == transaction_id =>
            {
                page.finish(finished.success, &finished.message);
                break;
            }
            Ok(BackupEvent::Alert(alert)) => page.status.set_label(
                &gettext("Alerta em {config} após {count} falha(s): {message}")
                    .replace("{config}", &alert.config_id)
                    .replace("{count}", &alert.consecutive_failures.to_string())
                    .replace("{message}", &alert.message),
            ),
            Ok(_) => {}
            Err(error) => {
                page.finish(false, &error.to_string());
                break;
            }
        }
    }
}

async fn monitor_backup_transaction(
    page: &crate::ui::BackupPage,
    events: &mut lyra_vega_dbus::BackupEventStream,
    transaction_id: u32,
) {
    loop {
        match events.next().await {
            Ok(BackupEvent::BackupProgress(progress))
                if progress.transaction_id == transaction_id =>
            {
                page.update_progress(progress.percent, &progress.message);
            }
            Ok(BackupEvent::BackupFinished(finished))
                if finished.transaction_id == transaction_id =>
            {
                page.finish(finished.success, &finished.message);
                if crate::preferences::notifications().2 {
                    send_notification(
                        &gettext("Backup concluído"),
                        &finished.message,
                        "backup-finished",
                    );
                }
                break;
            }
            Ok(BackupEvent::Alert(alert)) => page.status.set_label(
                &gettext("Alerta em {config} após {count} falha(s): {message}")
                    .replace("{config}", &alert.config_id)
                    .replace("{count}", &alert.consecutive_failures.to_string())
                    .replace("{message}", &alert.message),
            ),
            Ok(_) => {}
            Err(error) => {
                page.finish(false, &error.to_string());
                break;
            }
        }
    }
}

fn configure_software(shell: &VegaShell, window: &adw::ApplicationWindow, dbus: VegaDbus) {
    let dashboard_updates = shell.dashboard_updates.clone();
    watch_dashboard_updates(dashboard_updates.clone(), dbus.clone());

    let page = shell.software.clone();
    let button = page.search.clone();
    page.query.connect_activate(move |_| button.emit_clicked());

    let search_page = page.clone();
    page.search_tab.connect_clicked(move |button| {
        if button.is_active() {
            search_page.select_search();
        }
    });

    let installed_page = page.clone();
    let installed_dbus = dbus.clone();
    page.installed_tab.connect_clicked(move |button| {
        if !button.is_active() {
            return;
        }
        installed_page.select_installed();
        installed_page.set_busy(true);
        let page = installed_page.clone();
        let client = installed_dbus.software();
        glib::MainContext::default().spawn_local(async move {
            match client.list_installed().await {
                Ok(packages) => page.show_results(packages, false),
                Err(error) => page.show_error(&error.to_string()),
            }
            page.set_busy(false);
        });
    });

    // A busca da lista fica registrada na própria página em vez de morar só
    // no handler de `clicked`: abrir o Vega pela bandeja ou pela notificação
    // seleciona a aba por código, e `set_active` não emite `clicked` — sem
    // isto a aba ficava parada em "Verificando atualizações…" para sempre.
    let reload_page = page.clone();
    let reload_dbus = dbus.clone();
    page.set_updates_reloader(std::rc::Rc::new(move || {
        reload_page.set_busy(true);
        let page = reload_page.clone();
        let client = reload_dbus.software();
        glib::MainContext::default().spawn_local(async move {
            match client.list_updates().await {
                Ok(packages) => page.show_results(label_update_rows(packages), true),
                Err(error) => page.show_error(&error.to_string()),
            }
            page.set_busy(false);
        });
    }));

    let updates_page = page.clone();
    page.updates_tab.connect_clicked(move |button| {
        if !button.is_active() {
            return;
        }
        // select_updates dispara o recarregamento acima.
        updates_page.select_updates();
    });

    let repositories_page = page.clone();
    let repositories_dbus = dbus.clone();
    page.repositories_tab.connect_clicked(move |button| {
        if !button.is_active() {
            return;
        }
        repositories_page.select_repositories();
        repositories_page.repository_list.set_sensitive(false);
        let page = repositories_page.clone();
        let client = repositories_dbus.software();
        glib::MainContext::default().spawn_local(async move {
            match client.list_repos().await {
                Ok(repositories) => {
                    page.show_repositories(&repositories);
                    page.repository_list.set_sensitive(true);
                }
                Err(error) => page.finish_transaction(false, &error.to_string()),
            }
        });
    });

    connect_repository_toggle(&page, &dbus);
    connect_add_repo(&page, &dbus, &dashboard_updates);
    connect_update_package(&page, &dbus, &dashboard_updates);
    connect_install_queue(&page, &dbus, &dashboard_updates);

    let global_page = page.clone();
    let global_dbus = dbus.clone();
    let global_dashboard_updates = dashboard_updates.clone();
    page.global_action.connect_clicked(move |_| {
        let update_all = global_page.updates_tab.is_active();
        let (heading, body, confirm, starting) = if update_all {
            (
                gettext("Atualizar tudo?"),
                gettext("Executar a atualização completa do sistema e dos Flatpaks agora?"),
                gettext("Atualizar"),
                gettext("Iniciando atualização completa…"),
            )
        } else {
            (
                gettext("Limpar cache?"),
                gettext("Remover o cache de pacotes e runtimes Flatpak órfãos agora?"),
                gettext("Limpar"),
                gettext("Iniciando limpeza de cache…"),
            )
        };
        let dialog = adw::AlertDialog::new(Some(&heading), Some(&body));
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &confirm)]);
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let page = global_page.clone();
        let client = global_dbus.software();
        let dashboard_updates = global_dashboard_updates.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.global_action.set_sensitive(false);
            page.begin_transaction(&starting);
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => {
                    page.finish_transaction(false, &error.to_string());
                    page.global_action.set_sensitive(true);
                    return;
                }
            };
            let transaction = if update_all {
                client.update_all().await
            } else {
                client.clear_cache().await
            };
            match transaction {
                Ok(id) => {
                    monitor_software_transaction(
                        &page,
                        &client,
                        &mut events,
                        id,
                        &dashboard_updates,
                    )
                    .await
                }
                Err(error) => page.finish_transaction(false, &error.to_string()),
            }
            page.global_action.set_sensitive(true);
        });
    });

    let page_for_click = page.clone();
    let search_dbus = dbus.clone();
    page.search.connect_clicked(move |_| {
        let query = page_for_click.query.text().trim().to_owned();
        if query.chars().count() < 2 {
            page_for_click
                .status
                .set_label(&gettext("Digite ao menos dois caracteres para buscar"));
            return;
        }

        let page = page_for_click.clone();
        let client = search_dbus.software();
        page.set_busy(true);
        page.status
            .set_label(&gettext("Consultando as origens disponíveis…"));
        glib::MainContext::default().spawn_local(async move {
            match client.search(&query).await {
                Ok(packages) => page.show_results(packages, false),
                Err(error) => page.show_error(&error.to_string()),
            }
            page.set_busy(false);
        });
    });

    let page_for_selection = page.clone();
    let details_dbus = dbus.clone();
    let details_window = window.clone();
    page.connect_package_selected(move || {
        let Some(package) = page_for_selection.selected_package() else {
            return;
        };
        let page = page_for_selection.clone();
        let client = details_dbus.software();
        page.detail_title
            .set_label(&gettext("Carregando detalhes…"));
        page.detail_body
            .set_label(&gettext("Consultando a origem selecionada…"));
        page.action.set_sensitive(false);
        page.present_details(&details_window);
        glib::MainContext::default().spawn_local(async move {
            match client.package_details(&package.origin, &package.id).await {
                Ok(details) => page.show_details(&details),
                Err(error) => page.show_detail_error(&error.to_string()),
            }
        });
    });

    let action_page = page;
    let action_dbus = dbus;
    shell.software.action.connect_clicked(move |_| {
        let Some(package) = action_page.selected_package() else {
            return;
        };
        let page = action_page.clone();
        let client = action_dbus.software();
        let dashboard_updates = dashboard_updates.clone();
        glib::MainContext::default().spawn_local(async move {
            let verb = if package.installed {
                gettext("Remover")
            } else {
                gettext("Instalar")
            };
            page.action.set_sensitive(false);
            let confirmed = confirm_package_action(&package.name, &verb, package.installed).await;
            if !confirmed {
                page.action.set_label(&verb);
                page.action.set_sensitive(true);
                return;
            }
            page.action.set_label(&gettext("Iniciando…"));
            page.begin_transaction(
                &gettext("{verb} {name}")
                    .replace("{verb}", &verb)
                    .replace("{name}", &package.name),
            );
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => {
                    page.show_detail_error(&error.to_string());
                    return;
                }
            };
            let transaction_id = if package.installed {
                client.remove(&package.origin, &package.id).await
            } else {
                client.install(&package.origin, &package.id).await
            };
            let transaction_id = match transaction_id {
                Ok(id) => id,
                Err(error) => {
                    page.show_detail_error(&error.to_string());
                    return;
                }
            };

            loop {
                match events.next().await {
                    Ok(SoftwareEvent::Progress(progress))
                        if progress.transaction_id == transaction_id =>
                    {
                        page.show_transaction_progress(progress.percent, &progress.message);
                    }
                    Ok(SoftwareEvent::PackageProgress(progress))
                        if progress.transaction_id == transaction_id =>
                    {
                        page.show_package_progress(
                            &progress.package,
                            &progress.phase,
                            progress.percent,
                        );
                    }
                    Ok(SoftwareEvent::Finished(finished))
                        if finished.transaction_id == transaction_id =>
                    {
                        page.detail_body.set_label(&finished.message);
                        page.finish_transaction(finished.success, &finished.message);
                        page.action.set_label(&if finished.success {
                            gettext("Concluído")
                        } else {
                            gettext("Falhou")
                        });
                        page.action.set_sensitive(!finished.success);
                        if finished.success {
                            refresh_current_software_page(&page, &client, &dashboard_updates).await;
                        }
                        break;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        page.show_detail_error(&error.to_string());
                        break;
                    }
                }
            }
        });
    });
}

fn configure_nvidia(shell: &VegaShell, dbus: VegaDbus) {
    let status_page = shell.clone();
    let status_client = dbus.software();
    glib::MainContext::default().spawn_local(async move {
        match status_client.nvidia_status().await {
            Ok(status) => {
                status_page
                    .nvidia_title
                    .set_label(&if status.gpu.is_empty() {
                        gettext("Driver NVIDIA opcional")
                    } else {
                        gettext("NVIDIA G06 — {gpu}").replace("{gpu}", &status.gpu)
                    });
                let secure_boot = match status.secure_boot.as_str() {
                    "enabled" => {
                        gettext("Secure Boot ativado; será usado o módulo aberto assinado.")
                    }
                    "disabled" => gettext("Secure Boot desativado."),
                    _ => gettext("Não foi possível determinar o estado do Secure Boot."),
                };
                status_page.nvidia_detail.set_label(
                    &gettext("{detail}\n{secure_boot}")
                        .replace("{detail}", &status.detail)
                        .replace("{secure_boot}", &secure_boot),
                );
                status_page.nvidia_install.set_sensitive(
                    status.supported && !status.installed && status.state != "unavailable",
                );
                status_page.nvidia_check.set_sensitive(status.installed);
            }
            Err(error) => {
                status_page.nvidia_detail.set_label(&error.to_string());
                status_page.nvidia_install.set_sensitive(false);
                status_page.nvidia_check.set_sensitive(false);
            }
        }
    });

    let install_page = shell.clone();
    let install_dbus = dbus.clone();
    shell.nvidia_install.connect_clicked(move |_| {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Instalar o driver NVIDIA G06?")),
            Some(&gettext(
                "O Vega criará um snapshot somente leitura, adicionará o repositório oficial da NVIDIA, instalará módulo assinado, userspace e firmware em conjunto e regenerará o initramfs. Será necessário reiniciar. Não desligue o computador durante a transação.",
            )),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("install", &gettext("Instalar"))]);
        dialog.set_response_appearance("install", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = install_page.clone();
        let client = install_dbus.software();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "install").await { return; }
            page.nvidia_install.set_sensitive(false);
            page.nvidia_progress.set_fraction(0.0);
            page.nvidia_progress
                .set_text(Some(&gettext("Preparando instalação NVIDIA G06…")));
            page.nvidia_progress.set_visible(true);
            page.nvidia_detail
                .set_label(&gettext("Preparando instalação NVIDIA G06…"));
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => {
                    page.nvidia_detail.set_label(&error.to_string());
                    page.nvidia_progress.set_visible(false);
                    page.nvidia_install.set_sensitive(true);
                    return;
                }
            };
            let transaction_id = match client.install_nvidia(true).await {
                Ok(id) => id,
                Err(error) => {
                    page.nvidia_detail.set_label(&error.to_string());
                    page.nvidia_progress.set_visible(false);
                    page.nvidia_install.set_sensitive(true);
                    return;
                }
            };
            loop {
                match events.next().await {
                    Ok(SoftwareEvent::Progress(progress)) if progress.transaction_id == transaction_id => {
                        page.nvidia_progress
                            .set_fraction(progress.percent as f64 / 100.0);
                        page.nvidia_progress.set_text(Some(&progress.message));
                        page.nvidia_detail.set_label(&progress.message);
                    }
                    Ok(SoftwareEvent::Finished(finished)) if finished.transaction_id == transaction_id => {
                        page.nvidia_progress
                            .set_fraction(if finished.success { 1.0 } else { 0.0 });
                        page.nvidia_progress.set_text(Some(&if finished.success {
                            gettext("Concluído")
                        } else {
                            gettext("Falhou")
                        }));
                        page.nvidia_detail.set_label(&finished.message);
                        page.nvidia_check.set_sensitive(finished.success);
                        page.nvidia_install.set_sensitive(!finished.success);
                        break;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        page.nvidia_detail.set_label(&error.to_string());
                        page.nvidia_progress.set_visible(false);
                        page.nvidia_install.set_sensitive(true);
                        break;
                    }
                }
            }
        });
    });

    let check_page = shell.clone();
    let check_dbus = dbus;
    shell.nvidia_check.connect_clicked(move |_| {
        let page = check_page.clone();
        let client = check_dbus.software();
        page.nvidia_check.set_sensitive(false);
        glib::MainContext::default().spawn_local(async move {
            match client.check_nvidia().await {
                Ok((success, message)) => {
                    page.nvidia_detail.set_label(&message);
                    page.nvidia_title.set_label(&if success {
                        gettext("Driver NVIDIA ativo")
                    } else {
                        gettext("Driver NVIDIA requer atenção")
                    });
                }
                Err(error) => page.nvidia_detail.set_label(&error.to_string()),
            }
            page.nvidia_check.set_sensitive(true);
        });
    });
}

fn configure_non_free_firmware(shell: &VegaShell, dbus: VegaDbus) {
    let status_page = shell.clone();
    let status_client = dbus.software();
    glib::MainContext::default().spawn_local(async move {
        match status_client.non_free_firmware_status().await {
            Ok(status) => {
                status_page.firmware_detail.set_label(&status.detail);
                status_page
                    .firmware_install
                    .set_sensitive(status.detected && !status.installed);
            }
            Err(error) => {
                status_page.firmware_detail.set_label(&error.to_string());
                status_page.firmware_install.set_sensitive(false);
            }
        }
    });

    let install_page = shell.clone();
    let install_dbus = dbus;
    shell.firmware_install.connect_clicked(move |_| {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Instalar firmware não livre?")),
            Some(&gettext(
                "O Vega instalará somente pacotes do repositório non-oss associados de forma inequívoca ao hardware detectado. Um snapshot somente leitura será criado antes da alteração.",
            )),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("install", &gettext("Instalar"))]);
        dialog.set_response_appearance("install", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = install_page.clone();
        let client = install_dbus.software();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "install").await { return; }
            page.firmware_install.set_sensitive(false);
            page.firmware_progress.set_visible(true);
            page.firmware_progress.set_fraction(0.0);
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => { page.firmware_detail.set_label(&error.to_string()); return; }
            };
            let transaction_id = match client.install_non_free_firmware(true).await {
                Ok(id) => id,
                Err(error) => { page.firmware_detail.set_label(&error.to_string()); return; }
            };
            loop {
                match events.next().await {
                    Ok(SoftwareEvent::Progress(progress)) if progress.transaction_id == transaction_id => {
                        page.firmware_progress.set_fraction(progress.percent as f64 / 100.0);
                        page.firmware_progress.set_text(Some(&progress.message));
                        page.firmware_detail.set_label(&progress.message);
                    }
                    Ok(SoftwareEvent::Finished(finished)) if finished.transaction_id == transaction_id => {
                        page.firmware_progress.set_fraction(if finished.success { 1.0 } else { 0.0 });
                        page.firmware_detail.set_label(&finished.message);
                        page.firmware_install.set_sensitive(!finished.success);
                        break;
                    }
                    Ok(_) => {}
                    Err(error) => { page.firmware_detail.set_label(&error.to_string()); break; }
                }
            }
        });
    });
}

fn connect_repository_toggle(page: &crate::ui::SoftwarePage, dbus: &VegaDbus) {
    let page = page.clone();
    let dbus = dbus.clone();
    page.clone().connect_repository_toggle(move |repository| {
        let enabled = !repository.enabled;
        let verb = if enabled {
            gettext("Ativar")
        } else {
            gettext("Desativar")
        };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("{verb} repositório?").replace("{verb}", &verb)),
            Some(&repository.name),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", &verb)]);
        dialog.set_response_appearance(
            "confirm",
            if enabled {
                adw::ResponseAppearance::Suggested
            } else {
                adw::ResponseAppearance::Default
            },
        );
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = page.clone();
        let client = dbus.software();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }
            page.repository_list.set_sensitive(false);
            page.begin_transaction(
                &gettext("{verb} {name}…")
                    .replace("{verb}", &verb)
                    .replace("{name}", &repository.name),
            );
            match client.set_repo_enabled(&repository.name, enabled).await {
                Ok(()) => {
                    page.finish_transaction(true, &gettext("Repositório alterado com sucesso"));
                    match client.list_repos().await {
                        Ok(repositories) => page.show_repositories(&repositories),
                        Err(error) => page.finish_transaction(false, &error.to_string()),
                    }
                }
                Err(error) => page.finish_transaction(false, &error.to_string()),
            }
            page.repository_list.set_sensitive(true);
        });
    });
}

fn connect_add_repo(
    page: &crate::ui::SoftwarePage,
    dbus: &VegaDbus,
    dashboard_updates: &gtk::Label,
) {
    let page = page.clone();
    let dbus = dbus.clone();
    let dashboard_updates = dashboard_updates.clone();
    page.clone().connect_add_repo(move |name, url| {
        let page = page.clone();
        let client = dbus.software();
        let dashboard_updates = dashboard_updates.clone();
        glib::MainContext::default().spawn_local(async move {
            page.clear_add_repo_form();
            page.add_repo_button.set_sensitive(false);
            page.begin_transaction(&gettext("Adicionando {name}…").replace("{name}", &name));
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => {
                    page.finish_transaction(false, &error.to_string());
                    page.add_repo_button.set_sensitive(true);
                    return;
                }
            };
            match client.add_repo(&name, &url).await {
                Ok(id) => {
                    monitor_add_repo_transaction(
                        &page,
                        &client,
                        &mut events,
                        id,
                        &dashboard_updates,
                    )
                    .await
                }
                Err(error) => page.finish_transaction(false, &error.to_string()),
            }
            page.add_repo_button.set_sensitive(true);
        });
    });
}

/// Wires the per-row/card "Atualizar" button shown on the Updates tab (see
/// SoftwarePage::connect_update_package) to a single-package UpdatePackage
/// transaction — the same confirm/subscribe/monitor shape as the main
/// action button, just scoped to one package instead of the whole system.
fn connect_update_package(
    page: &crate::ui::SoftwarePage,
    dbus: &VegaDbus,
    dashboard_updates: &gtk::Label,
) {
    let page = page.clone();
    let dbus = dbus.clone();
    let dashboard_updates = dashboard_updates.clone();
    page.clone().connect_update_package(move |package| {
        let page = page.clone();
        let client = dbus.software();
        let dashboard_updates = dashboard_updates.clone();
        glib::MainContext::default().spawn_local(async move {
            if !confirm_package_action(&package.name, &gettext("Atualizar"), false).await {
                return;
            }
            page.set_update_actions_sensitive(false);
            page.begin_transaction(
                &gettext("Atualizando {name}…").replace("{name}", &package.name),
            );
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => {
                    page.finish_transaction(false, &error.to_string());
                    page.set_update_actions_sensitive(true);
                    return;
                }
            };
            match client.update_package(&package.origin, &package.id).await {
                Ok(id) => {
                    monitor_software_transaction(
                        &page,
                        &client,
                        &mut events,
                        id,
                        &dashboard_updates,
                    )
                    .await
                }
                Err(error) => page.finish_transaction(false, &error.to_string()),
            }
            page.set_update_actions_sensitive(true);
        });
    });
}

/// Wires the top "Instalar" button (see SoftwarePage::install_queue_button)
/// to install every package queued via the per-row "add to install queue"
/// toggle, one at a time — each queued package still runs through Install
/// as its own transaction (same as a single manual install), so mixed
/// origins in the same queue just work. A failure partway through doesn't
/// stop the rest, mirroring UpdateAll's per-repo failure tolerance; every
/// failure is collected into one final status message.
fn connect_install_queue(
    page: &crate::ui::SoftwarePage,
    dbus: &VegaDbus,
    dashboard_updates: &gtk::Label,
) {
    let page = page.clone();
    let dbus = dbus.clone();
    let dashboard_updates = dashboard_updates.clone();
    page.install_queue_button.clone().connect_clicked(move |_| {
        let page = page.clone();
        let client = dbus.software();
        let dashboard_updates = dashboard_updates.clone();
        glib::MainContext::default().spawn_local(async move {
            let mut queue = page.install_queue();
            if queue.is_empty() {
                return;
            }
            // Flathub installs don't need polkit; running them first means the
            // one auth prompt for the zypper installs after them is the only
            // one the user sees for the whole batch.
            queue.sort_by_key(|package| package.origin != "flathub");
            let heading =
                gettext("Instalar {count} pacote(s)?").replace("{count}", &queue.len().to_string());
            let dialog = adw::AlertDialog::new(
                Some(&heading),
                Some(&gettext(
                    "Cada pacote será instalado em sua própria transação; pacotes do sistema pedem autorização do polkit.",
                )),
            );
            dialog.add_responses(&[
                ("cancel", &gettext("Cancelar")),
                ("confirm", &gettext("Instalar")),
            ]);
            dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            if !confirm_dialog(&dialog, "confirm").await {
                return;
            }

            page.set_queue_actions_sensitive(false);
            let mut events = match client.subscribe().await {
                Ok(events) => events,
                Err(error) => {
                    page.finish_transaction(false, &error.to_string());
                    page.set_queue_actions_sensitive(true);
                    return;
                }
            };

            let total = queue.len();
            let mut failures = Vec::new();
            for (index, package) in queue.iter().enumerate() {
                page.begin_transaction(
                    &gettext("Instalando {name} ({current}/{total})…")
                        .replace("{name}", &package.name)
                        .replace("{current}", &(index + 1).to_string())
                        .replace("{total}", &total.to_string()),
                );
                let transaction_id = match client.install(&package.origin, &package.id).await {
                    Ok(id) => id,
                    Err(error) => {
                        failures.push(format!("{}: {}", package.name, error));
                        continue;
                    }
                };
                loop {
                    match events.next().await {
                        Ok(SoftwareEvent::Progress(progress))
                            if progress.transaction_id == transaction_id =>
                        {
                            page.update_transaction(progress.percent, &progress.message);
                        }
                        Ok(SoftwareEvent::PackageProgress(progress))
                            if progress.transaction_id == transaction_id =>
                        {
                            page.show_package_progress(
                                &progress.package,
                                &progress.phase,
                                progress.percent,
                            );
                        }
                        Ok(SoftwareEvent::Finished(finished))
                            if finished.transaction_id == transaction_id =>
                        {
                            if !finished.success {
                                failures.push(format!("{}: {}", package.name, finished.message));
                            }
                            break;
                        }
                        Ok(_) => {}
                        Err(error) => {
                            failures.push(format!("{}: {}", package.name, error));
                            break;
                        }
                    }
                }
            }

            page.clear_install_queue();
            let success = failures.is_empty();
            let message = if success {
                gettext("{count} pacote(s) instalado(s) com sucesso")
                    .replace("{count}", &total.to_string())
            } else {
                gettext("Falha ao instalar {failed} de {total} pacote(s): {details}")
                    .replace("{failed}", &failures.len().to_string())
                    .replace("{total}", &total.to_string())
                    .replace("{details}", &failures.join("; "))
            };
            page.finish_transaction(success, &message);
            refresh_current_software_page(&page, &client, &dashboard_updates).await;
            page.set_queue_actions_sensitive(true);
        });
    });
}

/// Like monitor_software_transaction, but AddRepo's transaction can also
/// emit RepoKeyPending before finishing (unsuccessfully) — when that
/// happens, this asks the user whether to trust the discovered key instead
/// of just reporting the failure.
async fn monitor_add_repo_transaction(
    page: &crate::ui::SoftwarePage,
    client: &lyra_vega_dbus::ZbusSoftwareClient,
    events: &mut lyra_vega_dbus::SoftwareEventStream,
    transaction_id: u32,
    dashboard_updates: &gtk::Label,
) {
    let mut pending_key: Option<RepositoryKeyInfo> = None;
    loop {
        match events.next().await {
            Ok(SoftwareEvent::Progress(progress)) if progress.transaction_id == transaction_id => {
                page.update_transaction(progress.percent, &progress.message);
                page.append_transaction_console(
                    "status",
                    &format!("{}% {}", progress.percent, progress.message),
                );
            }
            Ok(SoftwareEvent::KeyPending(info)) if info.transaction_id == transaction_id => {
                pending_key = Some(info);
            }
            Ok(SoftwareEvent::Finished(finished)) if finished.transaction_id == transaction_id => {
                if finished.success {
                    page.finish_transaction(true, &finished.message);
                    refresh_current_software_page(page, client, dashboard_updates).await;
                } else if let Some(key_info) = pending_key {
                    page.finish_transaction(
                        false,
                        &gettext("Repositório adicionado — assinatura pendente de aprovação"),
                    );
                    confirm_and_trust_repo_key(page, client, key_info, dashboard_updates).await;
                } else {
                    page.finish_transaction(false, &finished.message);
                }
                break;
            }
            Ok(_) => {}
            Err(error) => {
                page.finish_transaction(false, &error.to_string());
                break;
            }
        }
    }
}

/// Shows the fingerprint/userId of a repo's signing key and, if approved,
/// calls TrustRepoKey — trust-on-first-use, the same level of verification
/// pacman/zypper/dnf already give when a human approves the equivalent
/// prompt at the terminal. keyId can be empty (pacman has no way to
/// discover an importable key — see distro.UntrustedKeyError in vegad), in
/// which case trusting means disabling signature verification for this one
/// repo instead of importing a real key, so the wording differs.
async fn confirm_and_trust_repo_key(
    page: &crate::ui::SoftwarePage,
    client: &lyra_vega_dbus::ZbusSoftwareClient,
    key_info: RepositoryKeyInfo,
    dashboard_updates: &gtk::Label,
) {
    let body = if key_info.key_id.is_empty() {
        gettext(
            "Não foi possível obter uma chave verificável para \"{repo}\" (assinado por: {user}). \
             Deseja continuar mesmo assim, sem verificação de assinatura?",
        )
        .replace("{repo}", &key_info.repo)
        .replace("{user}", &key_info.user_id)
    } else {
        gettext(
            "\"{repo}\" está assinado por:\n{user}\nImpressão digital: {fingerprint}\n\nConfiar nessa chave e continuar?",
        )
        .replace("{repo}", &key_info.repo)
        .replace("{user}", &key_info.user_id)
        .replace("{fingerprint}", &key_info.fingerprint)
    };
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Confiar na chave do repositório?")),
        Some(&body),
    );
    dialog.add_responses(&[
        ("cancel", &gettext("Cancelar")),
        ("confirm", &gettext("Confiar")),
    ]);
    dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    if !confirm_dialog(&dialog, "confirm").await {
        return;
    }

    page.begin_transaction(&gettext("Confiando na chave…"));
    let mut events = match client.subscribe().await {
        Ok(events) => events,
        Err(error) => {
            page.finish_transaction(false, &error.to_string());
            return;
        }
    };
    match client
        .trust_repo_key(&key_info.repo, &key_info.key_id)
        .await
    {
        Ok(id) => {
            monitor_software_transaction(page, client, &mut events, id, dashboard_updates).await
        }
        Err(error) => page.finish_transaction(false, &error.to_string()),
    }
}

async fn confirm_package_action(name: &str, verb: &str, destructive: bool) -> bool {
    let dialog = adw::AlertDialog::new(
        Some(
            &gettext("{verb} {name}?")
                .replace("{verb}", verb)
                .replace("{name}", name),
        ),
        Some(&gettext(
            "A operação será autorizada pelo polkit e acompanhada até a conclusão.",
        )),
    );
    dialog.add_responses(&[("cancel", &gettext("Cancelar")), ("confirm", verb)]);
    dialog.set_response_appearance(
        "confirm",
        if destructive {
            adw::ResponseAppearance::Destructive
        } else {
            adw::ResponseAppearance::Suggested
        },
    );
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.choose_future(gtk::Widget::NONE).await == "confirm"
}

async fn monitor_software_transaction(
    page: &crate::ui::SoftwarePage,
    client: &impl SoftwareClient,
    events: &mut lyra_vega_dbus::SoftwareEventStream,
    transaction_id: u32,
    dashboard_updates: &gtk::Label,
) {
    loop {
        match events.next().await {
            Ok(SoftwareEvent::Progress(progress)) if progress.transaction_id == transaction_id => {
                page.update_transaction(progress.percent, &progress.message);
            }
            Ok(SoftwareEvent::PackageProgress(progress))
                if progress.transaction_id == transaction_id =>
            {
                page.show_package_progress(&progress.package, &progress.phase, progress.percent);
                page.append_transaction_console(
                    "zypper",
                    &format!(
                        "{} {} {}%",
                        progress.phase, progress.package, progress.percent
                    ),
                );
            }
            // `event` and `stdout` are synthesized by vegad from the two
            // progress signals handled above. Ignore those duplicates, but
            // retain any future raw diagnostic channel (notably stderr).
            Ok(SoftwareEvent::ConsoleLine(line))
                if line.transaction_id == transaction_id
                    && !matches!(line.source.as_str(), "event" | "stdout") =>
            {
                page.append_transaction_console(&line.source, &line.line);
            }
            Ok(SoftwareEvent::ConsoleLine(line)) if line.transaction_id == transaction_id => {
                page.append_transaction_console(&line.source, &line.line);
            }
            Ok(SoftwareEvent::Finished(finished)) if finished.transaction_id == transaction_id => {
                page.finish_transaction(finished.success, &finished.message);
                // Refresh even on failure: a multi-step transaction (e.g.
                // UpdateAll running Zypper then Flatpak) can partially
                // succeed, and the Updates tab must not keep showing
                // packages that were, in fact, already installed.
                if page.updates_tab.is_active() {
                    page.clear_results();
                }
                refresh_current_software_page(page, client, dashboard_updates).await;
                break;
            }
            Ok(_) => {}
            Err(error) => {
                page.finish_transaction(false, &error.to_string());
                break;
            }
        }
    }
}

async fn refresh_current_software_page(
    page: &crate::ui::SoftwarePage,
    client: &impl SoftwareClient,
    dashboard_updates: &gtk::Label,
) {
    page.set_busy(true);
    if page.installed_tab.is_active() {
        match client.list_installed().await {
            Ok(packages) => page.show_results(packages, false),
            Err(error) => page.show_error(&error.to_string()),
        }
    } else if page.updates_tab.is_active() {
        match client.list_updates().await {
            Ok(packages) => page.show_results(label_update_rows(packages), true),
            Err(error) => page.show_error(&error.to_string()),
        }
    } else if page.repositories_tab.is_active() {
        match client.list_repos().await {
            Ok(repositories) => page.show_repositories(&repositories),
            Err(error) => page.show_error(&error.to_string()),
        }
    } else {
        let query = page.query.text().trim().to_owned();
        if query.chars().count() >= 2 {
            match client.search(&query).await {
                Ok(packages) => page.show_results(packages, false),
                Err(error) => page.show_error(&error.to_string()),
            }
        }
    }
    page.set_busy(false);
    refresh_dashboard_updates(dashboard_updates, client).await;
}

/// Limpa a contagem de pacotes pendentes e busca novamente junto ao vegad,
/// exibindo o resultado apenas se houver atualizações disponíveis.
/// Rotula as linhas de atualização do Flathub. O vegad não carrega texto de
/// interface — um app Flatpak pendente não tem descrição própria a mostrar
/// aqui, e a cópia precisa sair do catálogo traduzido do próprio app em vez
/// de vir fixa em português do daemon.
fn label_update_rows(mut packages: Vec<PackageRef>) -> Vec<PackageRef> {
    for package in &mut packages {
        if package.origin == "flathub" && package.description.is_empty() {
            package.description = gettext("Atualização disponível");
        }
    }
    packages
}

async fn refresh_dashboard_updates(dashboard_updates: &gtk::Label, client: &impl SoftwareClient) {
    dashboard_updates.set_label(&gettext("Verificando atualizações…"));
    match client.list_updates().await {
        Ok(updates) if updates.is_empty() => dashboard_updates.set_label(&gettext("Tudo em dia")),
        Ok(updates) => dashboard_updates.set_label(
            &gettext("{count} pacote(s) pendente(s)")
                .replace("{count}", &updates.len().to_string()),
        ),
        Err(error) => dashboard_updates.set_label(&error.to_string()),
    }
}

/// Escuta o sinal `UpdatesAvailable` emitido pela checagem periódica em segundo
/// plano do vegad e atualiza o resumo do painel quando novos pacotes surgirem.
fn watch_dashboard_updates(dashboard_updates: gtk::Label, dbus: VegaDbus) {
    glib::MainContext::default().spawn_local(async move {
        let client = dbus.software();
        let Ok(mut events) = client.subscribe().await else {
            return;
        };
        loop {
            match events.next().await {
                Ok(SoftwareEvent::UpdatesAvailable(_count)) => {
                    refresh_dashboard_updates(&dashboard_updates, &client).await;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
}

fn set_unavailable(shell: &VegaShell, message: &str) {
    shell.backend_status.set_label(message);
    shell
        .dashboard_system
        .set_label(&gettext("Backend indisponível"));
    shell.dashboard_updates.set_label(message);
    shell.dashboard_backup.set_label(message);
    shell.dashboard_snapshots.set_label(message);
    shell.dashboard_services.set_label(message);
    shell.dashboard_disk.set_label(message);
    shell.hardware_cpu.set_label(message);
    shell.hardware_gpu.set_label("—");
    shell.hardware_ram.set_label("—");
    shell.hardware_firmware.set_label("—");
}

fn send_notification(title: &str, body: &str, id: &str) {
    if let Some(app) = gio::Application::default() {
        let notification = gio::Notification::new(title);
        notification.set_body(Some(body));
        app.send_notification(Some(id), &notification);
    }
}

async fn confirm_dialog(dialog: &adw::AlertDialog, response: &str) -> bool {
    !crate::preferences::confirmations_enabled()
        || dialog.clone().choose_future(gtk::Widget::NONE).await == response
}

#[cfg(test)]
mod tests {
    use super::{APPLICATION_ID, valid_ipv4_cidr};

    #[test]
    fn production_id_is_stable() {
        assert_eq!(APPLICATION_ID, "org.lyraos.Vega.Xfce");
    }

    #[test]
    fn static_ipv4_requires_address_and_valid_prefix() {
        assert!(valid_ipv4_cidr("192.168.1.20/24"));
        assert!(valid_ipv4_cidr("10.0.0.1/32"));
        assert!(!valid_ipv4_cidr("192.168.1.20"));
        assert!(!valid_ipv4_cidr("192.168.1.20/33"));
        assert!(!valid_ipv4_cidr("not-an-address/24"));
    }
}
