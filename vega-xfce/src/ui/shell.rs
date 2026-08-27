use crate::i18n::gettext;
use adw::prelude::*;
use gtk::gio;
use std::{cell::RefCell, rc::Rc};

use super::{
    AssistantPage, BackupPage, BluetoothPage, DateTimePage, KernelPage, LogsPage, MonitorPage,
    NetworkPage, ScreenPage, ServicesPage, SnapshotsPage, SoftwarePage, StoragePage, UsersPage,
};

#[derive(Clone)]
pub struct VegaShell {
    pub root: gtk::Box,
    pub stack: gtk::Stack,
    pub backend_status: gtk::Label,
    pub dashboard_system: gtk::Label,
    pub dashboard_updates: gtk::Label,
    pub dashboard_backup: gtk::Label,
    pub dashboard_snapshots: gtk::Label,
    pub dashboard_services: gtk::Label,
    pub dashboard_disk: gtk::Label,
    pub hardware_cpu: gtk::Label,
    pub hardware_gpu: gtk::Label,
    pub hardware_ram: gtk::Label,
    pub hardware_firmware: gtk::Label,
    pub nvidia_title: gtk::Label,
    pub nvidia_detail: gtk::Label,
    pub nvidia_install: gtk::Button,
    pub nvidia_check: gtk::Button,
    pub nvidia_progress: gtk::ProgressBar,
    pub firmware_detail: gtk::Label,
    pub firmware_install: gtk::Button,
    pub firmware_progress: gtk::ProgressBar,
    pub software: SoftwarePage,
    pub backup: BackupPage,
    pub snapshots: SnapshotsPage,
    pub kernel: KernelPage,
    pub datetime: DateTimePage,
    pub storage: StoragePage,
    pub network: NetworkPage,
    pub bluetooth: BluetoothPage,
    pub services: ServicesPage,
    pub users: UsersPage,
    pub logs: LogsPage,
    pub assistant: AssistantPage,
    pub screen: ScreenPage,
    pub monitor: MonitorPage,
}

impl VegaShell {
    pub fn new() -> Self {
        let preferences = Rc::new(RefCell::new(crate::preferences::load()));
        let backend_status = status_label(&gettext("Conectando ao vegad…"));
        let dashboard_system = status_label(&gettext("Carregando informações do sistema…"));
        let dashboard_updates = status_label(&gettext("Carregando…"));
        let dashboard_backup = status_label(&gettext("Carregando…"));
        let dashboard_snapshots = status_label(&gettext("Carregando…"));
        let dashboard_services = status_label(&gettext("Carregando…"));
        let dashboard_disk = status_label(&gettext("Carregando…"));
        let hardware_cpu = value_label(&gettext("Carregando…"));
        let hardware_gpu = value_label(&gettext("Carregando…"));
        let hardware_ram = value_label(&gettext("Carregando…"));
        let hardware_firmware = value_label(&gettext("Carregando…"));
        let nvidia = nvidia_card();
        let firmware = non_free_firmware_card();
        let software = SoftwarePage::new();
        let backup = BackupPage::new();
        let snapshots = SnapshotsPage::new();
        let kernel = KernelPage::new();
        let datetime = DateTimePage::new();
        let storage = StoragePage::new();
        let network = NetworkPage::new();
        let bluetooth = BluetoothPage::new();
        let services = ServicesPage::new();
        let users = UsersPage::new();
        let logs = LogsPage::new();
        let assistant = AssistantPage::new(
            &crate::assistant::load_settings(),
            crate::assistant::load_history(),
        );
        let screen = ScreenPage::new();
        let monitor = MonitorPage::new();

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            // Each page must negotiate its own width. The default horizontal
            // homogeneity makes every page inherit the widest page's minimum
            // size, pushing otherwise responsive content past the viewport.
            .hhomogeneous(false)
            .hexpand(true)
            .vexpand(true)
            .build();
        stack.add_titled(
            &dashboard_page(
                &stack,
                DashboardWidgets {
                    backend: &backend_status,
                    system: &dashboard_system,
                    updates: &dashboard_updates,
                    backup: &dashboard_backup,
                    snapshots: &dashboard_snapshots,
                    services: &dashboard_services,
                    disk: &dashboard_disk,
                },
            ),
            Some("dashboard"),
            &gettext("Painel"),
        );
        stack.add_titled(&storage.root, Some("storage"), &gettext("Armazenamento"));
        stack.add_titled(&network.root, Some("network"), &gettext("Rede e Firewall"));
        stack.add_titled(&bluetooth.root, Some("desktop"), &gettext("Bluetooth"));
        stack.add_titled(&services.root, Some("services"), &gettext("Serviços"));
        stack.add_titled(&users.root, Some("users"), &gettext("Usuários"));
        stack.add_titled(&logs.root, Some("logs"), &gettext("Log do Sistema"));
        stack.add_titled(
            &assistant.root,
            Some("assistant"),
            &gettext("Assistente de IA"),
        );
        stack.add_titled(
            &datetime.root,
            Some("datetime"),
            &gettext("Data, Hora e Idioma"),
        );
        stack.add_titled(&screen.root, Some("screen"), &gettext("Personalização"));
        stack.add_titled(
            &monitor.root,
            Some("monitor"),
            &gettext("Monitor do Sistema"),
        );
        stack.add_titled(&software.root, Some("software"), &gettext("Software"));
        stack.add_titled(
            &tabbed_page(
                &gettext("Backup"),
                &gettext("Backups e pontos de restauração protegidos pelo vegad"),
                &[
                    (gettext("Backups"), backup.root.clone()),
                    (gettext("Pontos de Restauração"), snapshots.root.clone()),
                ],
            ),
            Some("backup"),
            &gettext("Backup"),
        );
        stack.add_titled(
            &tabbed_page(
                &gettext("Hardware e Kernel"),
                &gettext("Inventário e kernel detectados pelo vegad"),
                &[
                    (
                        gettext("Hardware"),
                        hardware_page(
                            &hardware_cpu,
                            &hardware_gpu,
                            &hardware_ram,
                            &hardware_firmware,
                            &nvidia.root,
                            &firmware.root,
                        ),
                    ),
                    (gettext("Kernel"), kernel.root.clone()),
                ],
            ),
            Some("hardware"),
            &gettext("Hardware e Kernel"),
        );
        let brand = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        brand.add_css_class("sidebar-brand");
        let mark = gtk::Label::new(Some(" "));
        mark.add_css_class("brand-mark");
        brand.append(&mark);
        brand.append(&gtk::Label::new(Some("Vega")));
        let sidebar_search = gtk::SearchEntry::builder()
            .placeholder_text(gettext("Buscar configuração…"))
            .build();
        sidebar_search.add_css_class("sidebar-search");
        // Placeholder sozinho não vira nome acessível (confirmado via
        // inspeção AT-SPI: a entrada aparecia sem "name" pro leitor de
        // tela) — expõe o mesmo texto como Property::Label.
        sidebar_search.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Buscar configuração",
        ))]);

        let nav = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let mut searchable = Vec::new();
        let mut nav_group = None;
        add_nav_section(
            &nav,
            &gettext("Principal"),
            &[
                (gettext("Painel"), "dashboard", "view-grid-symbolic"),
                (
                    gettext("Software"),
                    "software",
                    "system-software-install-symbolic",
                ),
                (gettext("Backup"), "backup", "document-save-symbolic"),
                (
                    gettext("Assistente de IA"),
                    "assistant",
                    "system-search-symbolic",
                ),
            ],
            &stack,
            &mut searchable,
            &mut nav_group,
        );
        add_nav_section(
            &nav,
            &gettext("Sistema"),
            &[
                (
                    gettext("Hardware e Kernel"),
                    "hardware",
                    "computer-symbolic",
                ),
                (
                    gettext("Data, Hora e Idioma"),
                    "datetime",
                    "preferences-system-time-symbolic",
                ),
                (
                    gettext("Personalização"),
                    "screen",
                    "preferences-desktop-wallpaper-symbolic",
                ),
                (
                    gettext("Monitor do Sistema"),
                    "monitor",
                    "power-profile-performance-symbolic",
                ),
                (
                    gettext("Armazenamento"),
                    "storage",
                    "drive-harddisk-system-symbolic",
                ),
                (
                    gettext("Rede e Firewall"),
                    "network",
                    "network-wireless-symbolic",
                ),
                (gettext("Bluetooth"), "desktop", "bluetooth-symbolic"),
                (gettext("Serviços"), "services", "system-run-symbolic"),
                (gettext("Usuários"), "users", "system-users-symbolic"),
                (gettext("Log do Sistema"), "logs", "text-x-generic-symbolic"),
            ],
            &stack,
            &mut searchable,
            &mut nav_group,
        );
        let start_page = preferences.borrow().start_page.clone();
        if let Some((_, target, button, _)) = searchable
            .iter()
            .find(|(_, target, _, _)| target == &start_page)
            .or_else(|| searchable.first())
        {
            button.set_active(true);
            stack.set_visible_child_name(target);
        }
        let nav_buttons = searchable.clone();
        stack.connect_visible_child_name_notify(move |stack| {
            let active = stack.visible_child_name().unwrap_or_default();
            for (_, target, button, section) in &nav_buttons {
                let is_active = target == active.as_str();
                button.set_active(is_active);
                if is_active {
                    section.set_expanded(true);
                }
            }
        });
        sidebar_search.connect_search_changed(move |entry| {
            let query = entry.text().to_lowercase();
            for (label, _, button, section) in &searchable {
                let matches = query.is_empty() || label.to_lowercase().contains(&query);
                button.set_visible(matches);
                if !query.is_empty() && matches {
                    section.set_expanded(true);
                }
            }
        });
        let sidebar_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_container.add_css_class("vega-sidebar");
        sidebar_container.append(&brand);
        sidebar_container.append(&sidebar_search);
        sidebar_container.append(&nav);

        let split = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .start_child(&sidebar_container)
            .end_child(&stack)
            .resize_start_child(false)
            .shrink_start_child(false)
            .position(240)
            .vexpand(true)
            .build();

        let title = adw::WindowTitle::new("Vega", "");
        let header = adw::HeaderBar::builder().title_widget(&title).build();
        header.add_css_class("window-chrome");
        header.pack_end(&app_menu(&stack, preferences));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("app-frame");
        root.append(&header);
        root.append(&split);

        Self {
            root,
            stack,
            backend_status,
            dashboard_system,
            dashboard_updates,
            dashboard_backup,
            dashboard_snapshots,
            dashboard_services,
            dashboard_disk,
            hardware_cpu,
            hardware_gpu,
            hardware_ram,
            hardware_firmware,
            nvidia_title: nvidia.title,
            nvidia_detail: nvidia.detail,
            nvidia_install: nvidia.install,
            nvidia_check: nvidia.check,
            nvidia_progress: nvidia.progress,
            firmware_detail: firmware.detail,
            firmware_install: firmware.install,
            firmware_progress: firmware.progress,
            software,
            backup,
            snapshots,
            kernel,
            datetime,
            storage,
            network,
            bluetooth,
            services,
            users,
            logs,
            assistant,
            screen,
            monitor,
        }
    }
}

fn app_menu(
    stack: &gtk::Stack,
    preferences: Rc<RefCell<crate::preferences::Settings>>,
) -> gtk::MenuButton {
    let menu = gio::Menu::new();
    let settings_section = gio::Menu::new();
    settings_section.append(Some(&gettext("Configurações")), Some("menu.settings"));
    menu.append_section(None, &settings_section);
    let about_section = gio::Menu::new();
    about_section.append(Some(&gettext("Sobre o Vega")), Some("menu.about"));
    menu.append_section(None, &about_section);

    let actions = gio::SimpleActionGroup::new();
    let settings = gio::SimpleAction::new("settings", None);
    let settings_stack = stack.clone();
    settings.connect_activate(move |_, _| {
        if let Some(window) = settings_stack
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok())
        {
            show_preferences(&window, preferences.clone());
        }
    });
    actions.add_action(&settings);

    let about = gio::SimpleAction::new("about", None);
    let about_stack = stack.clone();
    about.connect_activate(move |_, _| {
        let dialog = adw::AboutDialog::builder()
            .application_name("Vega")
            .application_icon("vega")
            .developer_name("Lyra OS")
            .version(crate::model::APPLICATION_VERSION)
            .website("https://github.com/lyra-os-linux/vega")
            .issue_url("https://github.com/lyra-os-linux/vega/issues")
            .license_type(gtk::License::Gpl30)
            .build();
        dialog.set_developers(&["Rodrigo Brito"]);
        if let Some(window) = about_stack
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok())
        {
            dialog.present(Some(&window));
        }
    });
    actions.add_action(&about);

    let button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text(gettext("Menu principal"))
        .build();
    button.insert_action_group("menu", Some(&actions));
    button.update_property(&[gtk::accessible::Property::Label(&gettext("Menu principal"))]);
    button
}

fn show_preferences(parent: &gtk::Window, preferences: Rc<RefCell<crate::preferences::Settings>>) {
    let dialog = adw::PreferencesDialog::builder()
        .title(gettext("Configurações"))
        .build();
    let page = adw::PreferencesPage::new();

    let general = adw::PreferencesGroup::builder()
        .title(gettext("Geral"))
        .build();

    const START_PAGES: [&str; 4] = ["dashboard", "software", "monitor", "assistant"];
    let start_page = adw::ComboRow::builder()
        .title(gettext("Página inicial"))
        .model(&gtk::StringList::new(&[
            &gettext("Painel"),
            &gettext("Software"),
            &gettext("Monitor do Sistema"),
            &gettext("Assistente de IA"),
        ]))
        .selected(
            START_PAGES
                .iter()
                .position(|page| *page == preferences.borrow().start_page)
                .unwrap_or(0) as u32,
        )
        .build();
    general.add(&start_page);

    let refresh = adw::SpinRow::builder()
        .title(gettext("Atualização automática"))
        .subtitle(gettext("Intervalo em minutos"))
        .adjustment(&gtk::Adjustment::new(
            preferences.borrow().refresh_interval_minutes.into(),
            1.0,
            60.0,
            1.0,
            5.0,
            0.0,
        ))
        .build();
    general.add(&refresh);
    let confirmations = adw::SwitchRow::builder()
        .title(gettext("Confirmar ações administrativas"))
        .subtitle(gettext(
            "Solicitar confirmação antes de alterações no sistema",
        ))
        .active(preferences.borrow().confirm_actions)
        .build();
    general.add(&confirmations);
    page.add(&general);

    let notifications = adw::PreferencesGroup::builder()
        .title(gettext("Notificações"))
        .build();
    let notify_updates = switch_row(
        &gettext("Atualizações disponíveis"),
        preferences.borrow().notify_updates,
    );
    let notify_services = switch_row(
        &gettext("Falhas em serviços"),
        preferences.borrow().notify_service_failures,
    );
    let notify_backups = switch_row(
        &gettext("Conclusão de backups"),
        preferences.borrow().notify_backups,
    );
    notifications.add(&notify_updates);
    notifications.add(&notify_services);
    notifications.add(&notify_backups);
    page.add(&notifications);

    let privacy = adw::PreferencesGroup::builder()
        .title(gettext("Privacidade da IA"))
        .build();
    let redact_ai = adw::SwitchRow::builder()
        .title(gettext("Ocultar dados sensíveis"))
        .subtitle(gettext(
            "Remove credenciais e identificadores antes do envio",
        ))
        .active(preferences.borrow().redact_ai_data)
        .build();
    let save_history = adw::SwitchRow::builder()
        .title(gettext("Salvar histórico local"))
        .subtitle(gettext("Mantém as conversas da IA neste dispositivo"))
        .active(preferences.borrow().save_ai_history)
        .build();
    privacy.add(&redact_ai);
    privacy.add(&save_history);
    page.add(&privacy);
    dialog.add(&page);

    macro_rules! save_on_change {
        ($widget:expr, $signal:ident, $update:expr) => {{
            let preferences = preferences.clone();
            $widget.$signal(move |widget| {
                let mut settings = preferences.borrow_mut();
                $update(&mut settings, widget);
                crate::preferences::save(&settings);
            });
        }};
    }

    save_on_change!(
        start_page,
        connect_selected_notify,
        |settings: &mut crate::preferences::Settings, row: &adw::ComboRow| {
            settings.start_page = START_PAGES
                .get(row.selected() as usize)
                .unwrap_or(&"dashboard")
                .to_string();
        }
    );
    save_on_change!(
        refresh,
        connect_value_notify,
        |settings: &mut crate::preferences::Settings, row: &adw::SpinRow| {
            settings.refresh_interval_minutes = row.value() as u32;
        }
    );
    save_on_change!(
        confirmations,
        connect_active_notify,
        |settings: &mut crate::preferences::Settings, row: &adw::SwitchRow| {
            settings.confirm_actions = row.is_active();
        }
    );
    save_on_change!(
        notify_updates,
        connect_active_notify,
        |settings: &mut crate::preferences::Settings, row: &adw::SwitchRow| {
            settings.notify_updates = row.is_active();
        }
    );
    save_on_change!(
        notify_services,
        connect_active_notify,
        |settings: &mut crate::preferences::Settings, row: &adw::SwitchRow| {
            settings.notify_service_failures = row.is_active();
        }
    );
    save_on_change!(
        notify_backups,
        connect_active_notify,
        |settings: &mut crate::preferences::Settings, row: &adw::SwitchRow| {
            settings.notify_backups = row.is_active();
        }
    );
    save_on_change!(
        redact_ai,
        connect_active_notify,
        |settings: &mut crate::preferences::Settings, row: &adw::SwitchRow| {
            settings.redact_ai_data = row.is_active();
        }
    );
    save_on_change!(
        save_history,
        connect_active_notify,
        |settings: &mut crate::preferences::Settings, row: &adw::SwitchRow| {
            settings.save_ai_history = row.is_active();
            if !settings.save_ai_history {
                let _ = crate::assistant::clear_history();
            }
        }
    );

    dialog.present(Some(parent));
}

fn switch_row(title: &str, active: bool) -> adw::SwitchRow {
    adw::SwitchRow::builder()
        .title(title)
        .active(active)
        .build()
}

struct DashboardWidgets<'a> {
    backend: &'a gtk::Label,
    system: &'a gtk::Label,
    updates: &'a gtk::Label,
    backup: &'a gtk::Label,
    snapshots: &'a gtk::Label,
    services: &'a gtk::Label,
    disk: &'a gtk::Label,
}

fn dashboard_page(stack: &gtk::Stack, widgets: DashboardWidgets<'_>) -> gtk::Widget {
    let content = page_box(&gettext("Painel"), &gettext("Visão geral do sistema"));
    let grid = gtk::FlowBox::builder()
        .column_spacing(8)
        .row_spacing(8)
        .min_children_per_line(1)
        .max_children_per_line(4)
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .build();
    grid.insert(
        &dashboard_card(&gettext("Backend"), widgets.backend, None, stack),
        -1,
    );
    grid.insert(
        &dashboard_card(&gettext("Sistema"), widgets.system, None, stack),
        -1,
    );
    grid.insert(
        &dashboard_card(
            &gettext("Atualizações"),
            widgets.updates,
            Some("software"),
            stack,
        ),
        -1,
    );
    grid.insert(
        &dashboard_card(&gettext("Backup"), widgets.backup, Some("backup"), stack),
        -1,
    );
    grid.insert(
        &dashboard_card(
            &gettext("Pontos de Restauração"),
            widgets.snapshots,
            Some("snapshots"),
            stack,
        ),
        -1,
    );
    grid.insert(
        &dashboard_card(&gettext("Serviços"), widgets.services, None, stack),
        -1,
    );
    grid.insert(
        &dashboard_card(&gettext("Disco (/)"), widgets.disk, Some("hardware"), stack),
        -1,
    );
    content.append(&grid);
    scrolled(content)
}

fn dashboard_card(
    title: &str,
    value: &gtk::Label,
    target: Option<&'static str>,
    stack: &gtk::Stack,
) -> gtk::Widget {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let title = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["dim-label", "card-title"])
        .build();
    body.append(&title);
    body.append(value);
    let button = gtk::Button::builder()
        .child(&body)
        .hexpand(true)
        .css_classes(["card", "dashboard-card"])
        .build();
    if let Some(target) = target {
        let stack = stack.clone();
        button.connect_clicked(move |_| stack.set_visible_child_name(target));
    }
    button.upcast()
}

fn add_nav_section(
    container: &gtk::Box,
    title: &str,
    items: &[(String, &'static str, &'static str)],
    stack: &gtk::Stack,
    searchable: &mut Vec<(String, String, gtk::ToggleButton, gtk::Expander)>,
    group: &mut Option<gtk::ToggleButton>,
) {
    let section_content = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let section = gtk::Expander::builder()
        .label(title)
        .expanded(true)
        .child(&section_content)
        .build();
    section.add_css_class("sidebar-expander");
    container.append(&section);
    for (label, target, icon_name) in items {
        let label = label.clone();
        let target = (*target).to_owned();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let icon = gtk::Image::builder()
            .icon_name(*icon_name)
            .pixel_size(16)
            .build();
        icon.add_css_class("sidebar-icon");
        row.append(&icon);
        row.append(
            &gtk::Label::builder()
                .label(&label)
                .xalign(0.0)
                .hexpand(true)
                .build(),
        );
        let button = gtk::ToggleButton::builder()
            .child(&row)
            .halign(gtk::Align::Fill)
            .css_classes(["flat", "sidebar-item"])
            .build();
        // O texto de navegação só existe no Label filho (dentro de `row`);
        // sem isso, o próprio botão focável (o item real da árvore de
        // acessibilidade) chega vazio pro leitor de tela — confirmado por
        // inspeção AT-SPI, 13 botões de navegação afetados.
        button.update_property(&[gtk::accessible::Property::Label(&label)]);
        if let Some(first) = group.as_ref() {
            button.set_group(Some(first));
        } else {
            *group = Some(button.clone());
        }
        let stack = stack.clone();
        let target_for_click = target.clone();
        button.connect_clicked(move |button| {
            if button.is_active() {
                stack.set_visible_child_name(&target_for_click);
            }
        });
        section_content.append(&button);
        searchable.push((label, target, button, section.clone()));
    }
}

struct NvidiaWidgets {
    root: gtk::Box,
    title: gtk::Label,
    detail: gtk::Label,
    install: gtk::Button,
    check: gtk::Button,
    progress: gtk::ProgressBar,
}

struct FirmwareWidgets {
    root: gtk::Box,
    detail: gtk::Label,
    install: gtk::Button,
    progress: gtk::ProgressBar,
}

fn non_free_firmware_card() -> FirmwareWidgets {
    let title = gtk::Label::builder()
        .label(gettext("Firmware não livre"))
        .xalign(0.0)
        .css_classes(["title-3"])
        .build();
    let detail = gtk::Label::builder()
        .label(gettext("Verificando firmware compatível…"))
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let install = gtk::Button::builder()
        .label(gettext("Instalar firmware compatível"))
        .halign(gtk::Align::Start)
        .sensitive(false)
        .css_classes(["suggested-action"])
        .build();
    let progress = gtk::ProgressBar::builder()
        .show_text(true)
        .visible(false)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.add_css_class("card");
    root.append(&title);
    root.append(&detail);
    root.append(&progress);
    root.append(&install);
    FirmwareWidgets {
        root,
        detail,
        install,
        progress,
    }
}

fn nvidia_card() -> NvidiaWidgets {
    let title = gtk::Label::builder()
        .label(gettext("Verificando hardware NVIDIA…"))
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .css_classes(["title-3"])
        .build();
    let detail = gtk::Label::builder()
        .label(gettext(
            "A instalação G06 é opcional e usa o repositório oficial da NVIDIA.",
        ))
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let install = gtk::Button::builder()
        .label(gettext("Instalar driver NVIDIA G06"))
        .sensitive(false)
        .css_classes(["suggested-action"])
        .build();
    let check = gtk::Button::builder()
        .label(gettext("Verificar driver"))
        .sensitive(false)
        .build();
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.append(&install);
    actions.append(&check);
    let progress = gtk::ProgressBar::builder()
        .show_text(true)
        .visible(false)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.add_css_class("card");
    root.append(&title);
    root.append(&detail);
    root.append(&progress);
    root.append(&actions);
    NvidiaWidgets {
        root,
        title,
        detail,
        install,
        check,
        progress,
    }
}

fn hardware_page(
    cpu: &gtk::Label,
    gpu: &gtk::Label,
    ram: &gtk::Label,
    firmware: &gtk::Label,
    nvidia: &gtk::Box,
    firmware_card: &gtk::Box,
) -> gtk::Widget {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .build();
    content.add_css_class("content-page");
    content.add_css_class("compact-page");
    let group = adw::PreferencesGroup::builder()
        .title(gettext("Componentes"))
        .build();
    group.add(&property_row(&gettext("Processador"), cpu));
    group.add(&property_row(&gettext("Vídeo"), gpu));
    group.add(&property_row(&gettext("Memória"), ram));
    group.add(&property_row(&gettext("Firmware"), firmware));
    content.append(&group);
    content.append(nvidia);
    content.append(firmware_card);
    scrolled(content)
}

/// Combina páginas já existentes em abas dentro de uma única entrada de
/// navegação (mesmo padrão do módulo Software) — o título/subtítulo fica só
/// aqui, cada aba não repete o próprio cabeçalho de página.
fn tabbed_page(title: &str, subtitle: &str, tabs: &[(String, gtk::Widget)]) -> gtk::Widget {
    let tab_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    tab_box.add_css_class("module-tabs");
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .hhomogeneous(false)
        .vexpand(true)
        .build();
    let mut group: Option<gtk::ToggleButton> = None;
    for (index, (label, widget)) in tabs.iter().enumerate() {
        let name = format!("tab-{index}");
        stack.add_named(widget, Some(&name));
        let button = gtk::ToggleButton::builder()
            .label(label)
            .css_classes(["flat", "module-tab"])
            .build();
        if let Some(first) = &group {
            button.set_group(Some(first));
        } else {
            button.set_active(true);
            group = Some(button.clone());
        }
        let target_stack = stack.clone();
        button.connect_clicked(move |button| {
            if button.is_active() {
                target_stack.set_visible_child_name(&name);
            }
        });
        tab_box.append(&button);
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.add_css_class("content-page");
    content.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["title-1"])
            .build(),
    );
    content.append(
        &gtk::Label::builder()
            .label(subtitle)
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build(),
    );
    content.append(&tab_box);
    content.append(&stack);
    content.upcast()
}

fn page_box(title: &str, description: &str) -> gtk::Box {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .build();
    content.add_css_class("content-page");
    let heading = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["title-1"])
        .build();
    let subtitle = gtk::Label::builder()
        .label(description)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    content.append(&heading);
    content.append(&subtitle);
    content
}

fn property_row(title: &str, value: &gtk::Label) -> adw::ActionRow {
    value.set_hexpand(true);
    value.set_halign(gtk::Align::Fill);
    value.set_xalign(1.0);
    value.set_wrap(true);
    value.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    value.set_ellipsize(gtk::pango::EllipsizeMode::None);
    value.set_width_chars(24);
    value.set_max_width_chars(48);
    let row = adw::ActionRow::builder()
        .title(title)
        .title_lines(1)
        .build();
    row.add_suffix(value);
    row
}

fn status_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .build()
}

fn value_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(1.0)
        .wrap(true)
        .max_width_chars(56)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .selectable(true)
        .build()
}

fn scrolled(content: gtk::Box) -> gtk::Widget {
    gtk::ScrolledWindow::builder()
        .child(&content)
        .hscrollbar_policy(gtk::PolicyType::Never)
        // The content must adapt to the viewport. Propagating its natural
        // width lets wide cards and forms grow every page past the window.
        .propagate_natural_width(false)
        .build()
        .upcast()
}
