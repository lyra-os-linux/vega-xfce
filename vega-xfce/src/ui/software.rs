use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

use crate::i18n::gettext;
use adw::prelude::*;

use lyra_vega_dbus::{PackageRef, RepositoryRef};

type SelectionHandlers = Rc<RefCell<Vec<Rc<dyn Fn()>>>>;
type RepositoryToggleHandlers = Rc<RefCell<Vec<Rc<dyn Fn(RepositoryRef)>>>>;
type AddRepoHandlers = Rc<RefCell<Vec<Rc<dyn Fn(String, String)>>>>;
type UpdatePackageHandlers = Rc<RefCell<Vec<Rc<dyn Fn(PackageRef)>>>>;
type UpdatesReloader = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

#[derive(Clone, Debug)]
struct PackageGroup {
    packages: Vec<PackageRef>,
    selected: usize,
}

impl PackageGroup {
    fn selected(&self) -> Option<&PackageRef> {
        self.packages.get(self.selected)
    }
}

#[derive(Clone)]
pub struct SoftwarePage {
    pub root: gtk::Widget,
    pub query: gtk::SearchEntry,
    pub search: gtk::Button,
    pub status: gtk::Label,
    pub results: gtk::ListBox,
    pub cards: gtk::FlowBox,
    pub detail_title: gtk::Label,
    pub detail_body: gtk::Label,
    pub action: gtk::Button,
    pub detail_dialog: adw::Dialog,
    pub search_tab: gtk::ToggleButton,
    pub installed_tab: gtk::ToggleButton,
    pub updates_tab: gtk::ToggleButton,
    pub repositories_tab: gtk::ToggleButton,
    pub search_controls: gtk::Box,
    pub results_area: gtk::Box,
    pub repository_panel: gtk::Box,
    pub repository_list: gtk::ListBox,
    pub add_repo_name: gtk::Entry,
    pub add_repo_url: gtk::Entry,
    pub add_repo_button: gtk::Button,
    pub global_action: gtk::Button,
    pub transaction_panel: gtk::Box,
    pub transaction_close: gtk::Button,
    pub transaction_label: gtk::Label,
    pub transaction_progress: gtk::ProgressBar,
    pub transaction_console: gtk::TextView,
    package_groups: Rc<RefCell<Vec<PackageGroup>>>,
    package_progress_bars: Rc<RefCell<HashMap<String, gtk::ProgressBar>>>,
    selected_group: Rc<Cell<Option<usize>>>,
    selection_handlers: SelectionHandlers,
    repository_toggle_handlers: RepositoryToggleHandlers,
    add_repo_handlers: AddRepoHandlers,
    update_handlers: UpdatePackageHandlers,
    update_buttons: Rc<RefCell<Vec<gtk::Button>>>,
    install_queue: Rc<RefCell<Vec<PackageRef>>>,
    install_queue_toggles: Rc<RefCell<Vec<gtk::ToggleButton>>>,
    pub install_queue_button: gtk::Button,
    /// Recarrega a aba de atualizações. Registrado por `configure_software`
    /// assim que o cliente D-Bus existe, porque a página é construída antes
    /// dele. Guardar o recarregamento aqui é o que permite a navegação
    /// externa (ícone da bandeja, notificação) buscar a lista de verdade em
    /// vez de depender do sinal `clicked` da aba, que só um clique real
    /// emite.
    reload_updates: UpdatesReloader,
}

impl SoftwarePage {
    pub fn new() -> Self {
        let query = gtk::SearchEntry::builder()
            .placeholder_text(gettext("Buscar aplicativos e pacotes"))
            .hexpand(true)
            .build();
        let search = gtk::Button::builder()
            .label(gettext("Buscar"))
            .css_classes(["suggested-action"])
            .build();
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.append(&query);
        controls.append(&search);

        let search_tab = tab_button(&gettext("Buscar"));
        let installed_tab = tab_button(&gettext("Instalados"));
        let updates_tab = tab_button(&gettext("Atualizações"));
        let repositories_tab = tab_button(&gettext("Repositórios"));
        installed_tab.set_group(Some(&search_tab));
        updates_tab.set_group(Some(&search_tab));
        repositories_tab.set_group(Some(&search_tab));
        search_tab.set_active(true);
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabs.add_css_class("module-tabs");
        tabs.append(&search_tab);
        tabs.append(&installed_tab);
        tabs.append(&updates_tab);
        tabs.append(&repositories_tab);
        let global_action = gtk::Button::builder()
            .label(gettext("Limpar cache"))
            .halign(gtk::Align::End)
            .hexpand(true)
            .build();
        let install_queue_button = gtk::Button::builder()
            .label(gettext("Instalar"))
            .css_classes(["suggested-action"])
            .visible(false)
            .build();
        let list_view = gtk::ToggleButton::builder()
            .icon_name("view-list-symbolic")
            .tooltip_text(gettext("Visualização em lista"))
            .css_classes(["flat", "view-switch"])
            .active(true)
            .build();
        let card_view = gtk::ToggleButton::builder()
            .icon_name("view-grid-symbolic")
            .tooltip_text(gettext("Visualização em cartões"))
            .css_classes(["flat", "view-switch"])
            .build();
        card_view.set_group(Some(&list_view));
        tabs.append(&list_view);
        tabs.append(&card_view);
        tabs.append(&global_action);
        tabs.append(&install_queue_button);
        let transaction_label = gtk::Label::builder()
            .label(gettext("Preparando transação…"))
            .xalign(0.0)
            .wrap(true)
            .hexpand(true)
            .build();
        let transaction_close = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text(gettext("Fechar console"))
            .css_classes(["flat", "circular"])
            .valign(gtk::Align::Center)
            .visible(false)
            .build();
        let transaction_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        transaction_header.append(&transaction_label);
        transaction_header.append(&transaction_close);
        let transaction_progress = gtk::ProgressBar::builder()
            .show_text(true)
            .fraction(0.0)
            .build();
        let transaction_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        transaction_panel.add_css_class("card");
        transaction_panel.set_visible(false);
        transaction_panel.append(&transaction_header);
        transaction_panel.append(&transaction_progress);
        let panel = transaction_panel.clone();
        transaction_close.connect_clicked(move |_| panel.set_visible(false));
        let transaction_console = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::None)
            .top_margin(8)
            .bottom_margin(8)
            .left_margin(8)
            .right_margin(8)
            .css_classes(["transaction-console"])
            .build();
        let console_scroll = gtk::ScrolledWindow::builder()
            .child(&transaction_console)
            .min_content_height(180)
            .vexpand(false)
            .build();
        let console_expander = gtk::Expander::builder()
            .label(gettext("Console"))
            .child(&console_scroll)
            .expanded(true)
            .build();
        console_expander.connect_expanded_notify(|expander| {
            // The console lives inside a card in a vertically scrolling page.
            // Recompute the card immediately so collapsing it releases the
            // console's full height instead of retaining an empty allocation.
            expander.queue_resize();
        });
        transaction_panel.append(&console_expander);

        let status = gtk::Label::builder()
            .label(gettext("Digite ao menos dois caracteres para buscar"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let results = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .hexpand(true)
            .css_classes(["boxed-list"])
            .build();
        results.add_css_class("software-results");
        let cards = gtk::FlowBox::builder()
            .column_spacing(6)
            .row_spacing(6)
            .min_children_per_line(2)
            .max_children_per_line(4)
            .selection_mode(gtk::SelectionMode::Single)
            .homogeneous(true)
            .build();
        cards.add_css_class("software-cards");
        let result_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            // List and card views have very different size requests. Measure
            // only the active child so the hidden card grid cannot reserve
            // space beside/below the list (and vice versa).
            .hhomogeneous(false)
            .vhomogeneous(false)
            .build();
        result_stack.add_named(&results, Some("list"));
        result_stack.add_named(&cards, Some("cards"));
        result_stack.set_visible_child_name("list");
        let results_area = gtk::Box::new(gtk::Orientation::Vertical, 12);
        results_area.append(&status);
        results_area.append(&result_stack);

        let repository_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .hexpand(true)
            .css_classes(["boxed-list"])
            .build();
        let repository_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        repository_header.append(
            &gtk::Label::builder()
                .label(gettext("Repositórios do sistema"))
                .xalign(0.0)
                .hexpand(true)
                .css_classes(["title-3"])
                .build(),
        );
        let repository_panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
        repository_panel.add_css_class("card");
        repository_panel.set_visible(false);
        repository_panel.append(&repository_header);
        repository_panel.append(
            &gtk::Label::builder()
                .label(gettext(
                    "Selecione um repositório. A lista mostra o estado informado pelo gerenciador de pacotes da distribuição.",
                ))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );

        let add_repo_name = gtk::Entry::builder()
            .placeholder_text(gettext("nome"))
            .hexpand(true)
            .build();
        let add_repo_url = gtk::Entry::builder()
            .placeholder_text(gettext("URL"))
            .hexpand(true)
            .build();
        let add_repo_button = gtk::Button::builder()
            .label(gettext("Adicionar repositório"))
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();
        let add_repo_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        add_repo_row.append(&add_repo_name);
        add_repo_row.append(&add_repo_url);
        add_repo_row.append(&add_repo_button);
        let add_repo_form = gtk::Box::new(gtk::Orientation::Vertical, 8);
        add_repo_form.add_css_class("card");
        add_repo_form.append(
            &gtk::Label::builder()
                .label(gettext("Novo repositório"))
                .xalign(0.0)
                .css_classes(["title-3"])
                .build(),
        );
        add_repo_form.append(&add_repo_row);
        add_repo_form.append(
            &gtk::Label::builder()
                .label(gettext(
                    "Se o repositório for assinado, a chave é baixada e mostrada pra você aprovar antes de confiar nela.",
                ))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        repository_panel.append(&add_repo_form);
        repository_panel.append(&repository_list);
        let repository_toggle_handlers = RepositoryToggleHandlers::default();
        let list_stack = result_stack.clone();
        list_view.connect_clicked(move |button| {
            if button.is_active() {
                list_stack.set_visible_child_name("list");
            }
        });
        let card_stack = result_stack.clone();
        card_view.connect_clicked(move |button| {
            if button.is_active() {
                card_stack.set_visible_child_name("cards");
            }
        });
        let detail_title = gtk::Label::builder()
            .label(gettext("Selecione um pacote"))
            .xalign(0.0)
            .css_classes(["title-3"])
            .build();
        let detail_body = gtk::Label::builder()
            .label(gettext("Os detalhes e a ação disponível aparecerão aqui."))
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .build();
        let action = gtk::Button::builder()
            .label(gettext("Instalar"))
            .halign(gtk::Align::Start)
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();
        let detail_box = gtk::Box::new(gtk::Orientation::Vertical, 14);
        detail_box.add_css_class("package-detail-content");
        detail_box.append(&detail_title);
        detail_box.append(&detail_body);
        detail_box.append(&action);
        let detail_scroll = gtk::ScrolledWindow::builder()
            .child(&detail_box)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        let detail_header = adw::HeaderBar::builder()
            .title_widget(
                &gtk::Label::builder()
                    .label(gettext("Detalhes do pacote"))
                    .css_classes(["heading"])
                    .build(),
            )
            .build();
        let detail_layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
        detail_layout.append(&detail_header);
        detail_layout.append(&detail_scroll);
        let detail_dialog = adw::Dialog::builder()
            .child(&detail_layout)
            .content_width(680)
            .content_height(480)
            .build();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .build();
        content.add_css_class("content-page");
        content.append(
            &gtk::Label::builder()
                .label(gettext("Software"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(&tabs);
        content.append(&transaction_panel);
        content.append(&controls);
        content.append(&results_area);
        content.append(&repository_panel);

        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        let selected_group = Rc::new(Cell::new(None));
        let selection_handlers = Rc::new(RefCell::new(Vec::<Rc<dyn Fn()>>::new()));
        let list_selection = selected_group.clone();
        let list_handlers = selection_handlers.clone();
        results.connect_row_selected(move |_, row| {
            list_selection.set(row.map(|row| row.index() as usize));
            emit_selection(&list_handlers);
        });
        let card_selection = selected_group.clone();
        let card_handlers = selection_handlers.clone();
        cards.connect_child_activated(move |_, child| {
            card_selection.set(Some(child.index() as usize));
            emit_selection(&card_handlers);
        });

        let add_repo_handlers: AddRepoHandlers = Rc::new(RefCell::new(Vec::new()));
        let update_handlers: UpdatePackageHandlers = Rc::new(RefCell::new(Vec::new()));
        let sensitivity_button = add_repo_button.clone();
        let sensitivity_url = add_repo_url.clone();
        add_repo_name.connect_changed(move |entry| {
            sensitivity_button.set_sensitive(
                !entry.text().trim().is_empty() && !sensitivity_url.text().trim().is_empty(),
            );
        });
        let sensitivity_button = add_repo_button.clone();
        let sensitivity_name = add_repo_name.clone();
        add_repo_url.connect_changed(move |entry| {
            sensitivity_button.set_sensitive(
                !entry.text().trim().is_empty() && !sensitivity_name.text().trim().is_empty(),
            );
        });
        let click_name = add_repo_name.clone();
        let click_url = add_repo_url.clone();
        let click_handlers = add_repo_handlers.clone();
        add_repo_button.connect_clicked(move |_| {
            let name = click_name.text().trim().to_string();
            let url = click_url.text().trim().to_string();
            if name.is_empty() || url.is_empty() {
                return;
            }
            for handler in click_handlers.borrow().iter() {
                handler(name.clone(), url.clone());
            }
        });

        Self {
            root,
            query,
            search,
            status,
            results,
            cards,
            detail_title,
            detail_body,
            action,
            detail_dialog,
            search_tab,
            installed_tab,
            updates_tab,
            repositories_tab,
            search_controls: controls,
            results_area,
            repository_panel,
            repository_list,
            add_repo_name,
            add_repo_url,
            add_repo_button,
            global_action,
            transaction_panel,
            transaction_close,
            transaction_label,
            transaction_progress,
            transaction_console,
            package_groups: Rc::new(RefCell::new(Vec::new())),
            package_progress_bars: Rc::new(RefCell::new(HashMap::new())),
            selected_group,
            selection_handlers,
            repository_toggle_handlers,
            add_repo_handlers,
            update_handlers,
            update_buttons: Rc::new(RefCell::new(Vec::new())),
            install_queue: Rc::new(RefCell::new(Vec::new())),
            install_queue_toggles: Rc::new(RefCell::new(Vec::new())),
            install_queue_button,
            reload_updates: Rc::new(RefCell::new(None)),
        }
    }

    /// Define quem sabe buscar as atualizações e dispara a busca de imediato
    /// se a aba já estiver selecionada — é o caso de quem abriu o Vega pela
    /// bandeja ou pela notificação, cuja seleção acontece antes desta
    /// configuração.
    pub fn set_updates_reloader(&self, reload: Rc<dyn Fn()>) {
        *self.reload_updates.borrow_mut() = Some(reload.clone());
        if self.updates_tab.is_active() {
            reload();
        }
    }

    fn reload_updates(&self) {
        let reload = self.reload_updates.borrow().clone();
        if let Some(reload) = reload {
            reload();
        }
    }

    pub fn set_busy(&self, busy: bool) {
        self.query.set_sensitive(!busy);
        self.search.set_sensitive(!busy);
        self.search.set_label(&if busy {
            gettext("Buscando…")
        } else {
            gettext("Buscar")
        });
    }

    /// group_by_repository groups the list by each package's source
    /// repository (with section headers), in the order the backend already
    /// returned them — used for the Updates tab, where "Atualizar tudo" now
    /// runs repo by repo in that same order. Search/Installed pass false
    /// and keep the existing alphabetical-by-name grouping.
    pub fn show_results(&self, packages: Vec<PackageRef>, group_by_repository: bool) {
        self.clear_results();
        let groups = if group_by_repository {
            group_packages_preserving_order(packages)
        } else {
            group_packages(packages)
        };
        *self.package_groups.borrow_mut() = groups.clone();
        // group_by_repository só é verdadeiro na aba de atualizações, onde
        // "nenhum resultado" não descreve o que aconteceu: não houve busca
        // que falhasse, o sistema é que está em dia.
        self.status.set_label(&if groups.is_empty() {
            if group_by_repository {
                gettext("Nenhuma atualização disponível")
            } else {
                gettext("Nenhum resultado encontrado")
            }
        } else {
            gettext("Escolha a origem antes de instalar")
        });
        let mut repositories = Vec::with_capacity(groups.len());
        for (group_index, group) in groups.into_iter().enumerate() {
            let Some(package) = group.selected() else {
                continue;
            };
            repositories.push(package.repository.clone());
            let safe_name = gtk::glib::markup_escape_text(&package.name);
            let safe_description = gtk::glib::markup_escape_text(&package.description);
            let row = adw::ActionRow::builder()
                .title(safe_name)
                .subtitle(safe_description)
                .title_lines(1)
                .subtitle_lines(1)
                .activatable(true)
                .build();
            row.add_prefix(&package_icon(package, 34));
            if group.packages.iter().any(|package| package.installed) {
                row.add_suffix(&gtk::Label::new(Some(&gettext("Instalado"))));
            }
            let origins = origin_pills(
                group_index,
                &group,
                &self.package_groups,
                &self.selected_group,
                &self.selection_handlers,
            );
            row.add_suffix(&origins);
            let progress = gtk::ProgressBar::builder()
                .valign(gtk::Align::Center)
                .width_request(96)
                .show_text(true)
                .visible(false)
                .build();
            row.add_suffix(&progress);
            self.package_progress_bars
                .borrow_mut()
                .insert(package.name.clone(), progress);
            if group_by_repository {
                let button =
                    update_button(group_index, &self.package_groups, &self.update_handlers);
                self.update_buttons.borrow_mut().push(button.clone());
                row.add_suffix(&button);
            } else if !package.installed {
                let queued = self.is_queued(package);
                let toggle = install_queue_toggle(
                    group_index,
                    &self.package_groups,
                    &self.install_queue,
                    &self.install_queue_button,
                    queued,
                );
                self.install_queue_toggles.borrow_mut().push(toggle.clone());
                row.add_suffix(&toggle);
            }
            self.results.append(&row);

            let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
            card.add_css_class("software-card");
            let card_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            card_header.append(&package_icon(package, 36));
            card_header.append(
                &gtk::Label::builder()
                    .label(&package.name)
                    .xalign(0.0)
                    .hexpand(true)
                    .wrap(true)
                    .css_classes(["title-3"])
                    .build(),
            );
            card.append(&card_header);
            card.append(
                &gtk::Label::builder()
                    .label(&package.description)
                    .xalign(0.0)
                    .wrap(true)
                    .lines(2)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .css_classes(["dim-label"])
                    .build(),
            );
            card.append(&origin_pills(
                group_index,
                &group,
                &self.package_groups,
                &self.selected_group,
                &self.selection_handlers,
            ));
            if group_by_repository {
                let button =
                    update_button(group_index, &self.package_groups, &self.update_handlers);
                self.update_buttons.borrow_mut().push(button.clone());
                card.append(&button);
            } else if !package.installed {
                let queued = self.is_queued(package);
                let toggle = install_queue_toggle(
                    group_index,
                    &self.package_groups,
                    &self.install_queue,
                    &self.install_queue_button,
                    queued,
                );
                self.install_queue_toggles.borrow_mut().push(toggle.clone());
                card.append(&toggle);
            }
            self.cards.insert(&card, -1);
        }

        if group_by_repository {
            self.results.set_header_func(move |row, _before| {
                let index = row.index();
                if index < 0 {
                    row.set_header(None::<&gtk::Widget>);
                    return;
                }
                let index = index as usize;
                let current = repositories.get(index).map(String::as_str).unwrap_or("");
                let previous = index
                    .checked_sub(1)
                    .and_then(|i| repositories.get(i))
                    .map(String::as_str);
                if previous == Some(current) {
                    row.set_header(None::<&gtk::Widget>);
                    return;
                }
                let label = gtk::Label::builder()
                    .label(if current.is_empty() {
                        gettext("Outros")
                    } else {
                        current.to_string()
                    })
                    .xalign(0.0)
                    .css_classes(["heading"])
                    .build();
                row.set_header(Some(&label));
            });
        } else {
            self.results.unset_header_func();
        }
    }

    pub fn show_error(&self, message: &str) {
        self.status
            .set_label(&gettext("Falha: {message}").replace("{message}", message));
    }

    pub fn selected_package(&self) -> Option<PackageRef> {
        let index = self.selected_group.get()?;
        self.package_groups.borrow().get(index)?.selected().cloned()
    }

    pub fn connect_package_selected(&self, handler: impl Fn() + 'static) {
        self.selection_handlers.borrow_mut().push(Rc::new(handler));
    }

    pub fn present_details(&self, parent: &impl IsA<gtk::Widget>) {
        self.detail_dialog.present(Some(parent));
    }

    pub fn show_details(&self, details: &lyra_vega_dbus::PackageDetails) {
        self.detail_title.set_label(&format!(
            "{} • {}",
            details.name,
            origin_label(&details.origin)
        ));
        let licenses = if details.licenses.is_empty() {
            "—".to_string()
        } else {
            details.licenses.join(", ")
        };
        let body = gettext(
            "{description}\n\nDisponível: {available}  •  Instalado: {installed}\nDownload: {download}  •  Em disco: {disk}\nLicenças: {licenses}",
        )
        .replace("{description}", &details.description)
        .replace("{available}", value_or_dash(&details.available_version))
        .replace("{installed}", value_or_dash(&details.installed_version))
        .replace("{download}", value_or_dash(&details.download_size))
        .replace("{disk}", value_or_dash(&details.installed_size))
        .replace("{licenses}", &licenses);
        self.detail_body.set_label(&body);
        self.action.set_label(&if details.installed {
            gettext("Remover")
        } else {
            gettext("Instalar")
        });
        self.action.set_sensitive(true);
        if details.installed {
            self.action.remove_css_class("suggested-action");
            self.action.add_css_class("destructive-action");
        } else {
            self.action.remove_css_class("destructive-action");
            self.action.add_css_class("suggested-action");
        }
    }

    pub fn show_detail_error(&self, message: &str) {
        self.detail_title
            .set_label(&gettext("Detalhes indisponíveis"));
        self.detail_body.set_label(message);
        self.action.set_sensitive(false);
        present_error_alert(&self.detail_dialog, message);
    }

    /// Updates the per-row progress bar for `package` (matched by name —
    /// see PackageRef.repository/Id conventions for the "official" origin),
    /// revealing it on first use. Rows for packages the current transaction
    /// never touches (a different origin, or simply not in this list) just
    /// keep their bar hidden — see show_results, which pre-creates one bar
    /// per row regardless of whether it'll ever be driven.
    pub fn show_package_progress(&self, package: &str, phase: &str, percent: u32) {
        let bars = self.package_progress_bars.borrow();
        let Some(bar) = bars.get(package) else {
            return;
        };
        let percent = percent.min(100);
        bar.set_visible(true);
        bar.set_fraction(f64::from(percent) / 100.0);
        bar.set_text(Some(&format!("{} {percent}%", phase_label(phase))));
    }

    pub fn show_transaction_progress(&self, percent: u32, message: &str) {
        self.action.set_label(&format!("{percent}%"));
        self.detail_body.set_label(message);
        self.update_transaction(percent, message);
    }

    pub fn begin_transaction(&self, label: &str) {
        self.transaction_panel.set_visible(true);
        self.transaction_close.set_visible(false);
        self.transaction_label.set_label(label);
        self.transaction_progress.set_fraction(0.0);
        self.transaction_progress.set_text(Some("0%"));
        self.transaction_console
            .buffer()
            .set_text(&format!("[vega] {label}\n"));
    }

    pub fn append_transaction_console(&self, source: &str, line: &str) {
        let buffer = self.transaction_console.buffer();
        let mut text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        text.push_str(&format!("[{source}] {line}\n"));
        let mut lines: Vec<&str> = text.lines().collect();
        if lines.len() > 10_000 {
            lines.drain(..lines.len() - 10_000);
        }
        let mut bounded = lines.join("\n");
        if bounded.len() > 4 * 1024 * 1024 {
            let start = bounded.len() - 4 * 1024 * 1024;
            let start = bounded
                .char_indices()
                .find(|(index, _)| *index >= start)
                .map(|(index, _)| index)
                .unwrap_or(0);
            bounded = bounded[start..].to_string();
        }
        bounded.push('\n');
        buffer.set_text(&bounded);
        let mut end = buffer.end_iter();
        self.transaction_console
            .scroll_to_iter(&mut end, 0.0, false, 0.0, 1.0);
    }

    pub fn update_transaction(&self, percent: u32, message: &str) {
        let percent = percent.min(100);
        self.transaction_label.set_label(message);
        self.transaction_progress
            .set_fraction(f64::from(percent) / 100.0);
        self.transaction_progress
            .set_text(Some(&format!("{percent}%")));
    }

    pub fn finish_transaction(&self, success: bool, message: &str) {
        // Keep the transaction panel visible so fast zypper operations do
        // not make their console disappear before the user can inspect it.
        self.transaction_panel.set_visible(true);
        self.transaction_close.set_visible(true);
        self.transaction_label.set_label(message);
        if success {
            self.transaction_progress.set_fraction(1.0);
            self.transaction_progress.set_text(Some("100%"));
        }
        self.append_transaction_console(
            "vega",
            &format!(
                "{}: {message}",
                if success { "concluído" } else { "falhou" }
            ),
        );
        let status = if success {
            message.to_owned()
        } else {
            gettext("Falha: {message}").replace("{message}", message)
        };
        self.status.set_label(&status);
        if !success {
            present_error_alert(&self.root, &status);
        }
    }

    pub fn select_search(&self) {
        self.clear_results();
        self.search_controls.set_visible(true);
        self.results_area.set_visible(true);
        self.repository_panel.set_visible(false);
        self.global_action.set_visible(true);
        self.global_action.set_label(&gettext("Limpar cache"));
        self.status
            .set_label(&gettext("Digite ao menos dois caracteres para buscar"));
        update_install_queue_button(
            &self.install_queue_button,
            self.install_queue.borrow().len(),
        );
    }

    pub fn select_installed(&self) {
        self.clear_results();
        self.search_controls.set_visible(false);
        self.results_area.set_visible(true);
        self.repository_panel.set_visible(false);
        self.global_action.set_visible(false);
        self.install_queue_button.set_visible(false);
        self.status
            .set_label(&gettext("Carregando pacotes instalados…"));
    }

    pub fn select_updates(&self) {
        // Keep the tab selection in sync when navigation comes from an
        // external action (tray icon/notification), not only from a click on
        // the tab button itself.
        self.updates_tab.set_active(true);
        self.clear_results();
        self.search_controls.set_visible(false);
        self.results_area.set_visible(true);
        self.repository_panel.set_visible(false);
        self.global_action.set_visible(true);
        self.global_action.set_label(&gettext("Atualizar tudo"));
        self.install_queue_button.set_visible(false);
        self.status.set_label(&gettext("Verificando atualizações…"));
        self.reload_updates();
    }

    pub fn select_repositories(&self) {
        self.clear_results();
        self.search_controls.set_visible(false);
        self.results_area.set_visible(false);
        self.repository_panel.set_visible(true);
        self.global_action.set_visible(false);
        self.install_queue_button.set_visible(false);
    }

    pub fn show_repositories(&self, repositories: &[RepositoryRef]) {
        while let Some(child) = self.repository_list.first_child() {
            self.repository_list.remove(&child);
        }
        for repository in repositories {
            let row = adw::ActionRow::builder()
                .title(&repository.name)
                .subtitle(if repository.enabled {
                    gettext("Ativo")
                } else {
                    gettext("Inativo")
                })
                .build();
            let action = gtk::Button::builder()
                .label(if repository.enabled {
                    gettext("Desativar")
                } else {
                    gettext("Ativar")
                })
                .valign(gtk::Align::Center)
                .build();
            if !repository.enabled {
                action.add_css_class("suggested-action");
            }
            let selected_repository = repository.clone();
            let handlers = self.repository_toggle_handlers.clone();
            action.connect_clicked(move |_| {
                for handler in handlers.borrow().iter() {
                    handler(selected_repository.clone());
                }
            });
            row.add_suffix(&action);
            self.repository_list.append(&row);
        }
    }

    pub fn connect_repository_toggle(&self, callback: impl Fn(RepositoryRef) + 'static) {
        self.repository_toggle_handlers
            .borrow_mut()
            .push(Rc::new(callback));
    }

    /// callback receives (name, url) from the "Novo repositório" form.
    pub fn connect_add_repo(&self, callback: impl Fn(String, String) + 'static) {
        self.add_repo_handlers.borrow_mut().push(Rc::new(callback));
    }

    /// callback receives the package a per-row "Atualizar" button (Updates
    /// tab only — see show_results' group_by_repository branch) was clicked
    /// for, already resolved to whichever origin pill is selected.
    pub fn connect_update_package(&self, callback: impl Fn(PackageRef) + 'static) {
        self.update_handlers.borrow_mut().push(Rc::new(callback));
    }

    /// Disables every per-row "Atualizar" button and "Atualizar tudo" while
    /// one such transaction is running — zypper can't run two transactions
    /// at once (see zypperGroupedUpdates' concurrency note on the Go side).
    pub fn set_update_actions_sensitive(&self, sensitive: bool) {
        self.global_action.set_sensitive(sensitive);
        for button in self.update_buttons.borrow().iter() {
            button.set_sensitive(sensitive);
        }
    }

    fn is_queued(&self, package: &PackageRef) -> bool {
        self.install_queue
            .borrow()
            .iter()
            .any(|queued| queued.origin == package.origin && queued.id == package.id)
    }

    /// Snapshot of the packages queued via the per-row "add to install
    /// queue" toggle (Search tab only) — read by the top "Instalar" button's
    /// click handler to run the batch install.
    pub fn install_queue(&self) -> Vec<PackageRef> {
        self.install_queue.borrow().clone()
    }

    /// Empties the install queue and resets every toggle button still on
    /// screen back to its unqueued state — called once a batch install
    /// finishes (whether or not every package in it actually succeeded).
    pub fn clear_install_queue(&self) {
        self.install_queue.borrow_mut().clear();
        for toggle in self.install_queue_toggles.borrow().iter() {
            toggle.set_active(false);
        }
        update_install_queue_button(&self.install_queue_button, 0);
    }

    /// Disables the top "Instalar" button and every per-row queue toggle
    /// while a batch install is running.
    pub fn set_queue_actions_sensitive(&self, sensitive: bool) {
        self.install_queue_button.set_sensitive(sensitive);
        for toggle in self.install_queue_toggles.borrow().iter() {
            toggle.set_sensitive(sensitive);
        }
    }

    /// Clears the "Novo repositório" form — called after AddRepo starts (a
    /// successful transaction shouldn't leave stale text behind, and a
    /// failed one is cheap enough for the user to just retype).
    pub fn clear_add_repo_form(&self) {
        self.add_repo_name.set_text("");
        self.add_repo_url.set_text("");
        self.add_repo_button.set_sensitive(false);
    }

    pub fn clear_results(&self) {
        while let Some(child) = self.results.first_child() {
            self.results.remove(&child);
        }
        while let Some(child) = self.cards.first_child() {
            self.cards.remove(&child);
        }
        self.package_groups.borrow_mut().clear();
        self.package_progress_bars.borrow_mut().clear();
        self.update_buttons.borrow_mut().clear();
        self.install_queue_toggles.borrow_mut().clear();
        self.selected_group.set(None);
        self.action.set_sensitive(false);
        if self.detail_dialog.parent().is_some() {
            self.detail_dialog.close();
        }
    }
}

fn package_icon(package: &PackageRef, size: i32) -> gtk::Widget {
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class("package-icon");
    frame.set_width_request(size);
    frame.set_height_request(size);
    frame.set_hexpand(false);
    frame.set_vexpand(false);
    frame.set_overflow(gtk::Overflow::Hidden);
    frame.set_halign(gtk::Align::Center);
    frame.set_valign(gtk::Align::Center);
    let thumbnail_size = size - 6;
    let thumbnail = (!package.icon.is_empty() && std::path::Path::new(&package.icon).is_file())
        .then(|| {
            gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(
                &package.icon,
                thumbnail_size,
                thumbnail_size,
                true,
            )
            .ok()
        })
        .flatten();
    if let Some(thumbnail) = thumbnail {
        let image = gtk::Image::from_pixbuf(Some(&thumbnail));
        image.set_pixel_size(thumbnail_size);
        image.set_halign(gtk::Align::Center);
        image.set_valign(gtk::Align::Center);
        frame.append(&image);
    } else {
        let fallback = package
            .name
            .trim()
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        frame.append(
            &gtk::Label::builder()
                .label(&fallback)
                .css_classes(["package-icon-fallback"])
                .build(),
        );
    }
    frame.upcast()
}

fn emit_selection(handlers: &SelectionHandlers) {
    for handler in handlers.borrow().iter() {
        handler();
    }
}

fn origin_pills(
    group_index: usize,
    group: &PackageGroup,
    package_groups: &Rc<RefCell<Vec<PackageGroup>>>,
    selected_group: &Rc<Cell<Option<usize>>>,
    handlers: &SelectionHandlers,
) -> gtk::Box {
    let origins = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    origins.add_css_class("origin-pills");
    origins.set_valign(gtk::Align::Center);
    let mut first_button: Option<gtk::ToggleButton> = None;
    for (origin_index, candidate) in group.packages.iter().enumerate() {
        let origin = origin_label(&candidate.origin);
        let origin_text = gtk::Label::builder()
            .label(&origin)
            .max_width_chars(14)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let button = gtk::ToggleButton::builder()
            .css_classes(["flat", "origin-pill"])
            .tooltip_text(gettext("Usar a origem {origin}").replace("{origin}", &origin))
            .child(&origin_text)
            .build();
        if let Some(first) = &first_button {
            button.set_group(Some(first));
        } else {
            first_button = Some(button.clone());
        }
        button.set_active(origin_index == group.selected);
        let package_groups = package_groups.clone();
        let selected_group = selected_group.clone();
        let handlers = handlers.clone();
        button.connect_clicked(move |button| {
            if !button.is_active() {
                return;
            }
            if let Some(group) = package_groups.borrow_mut().get_mut(group_index) {
                group.selected = origin_index;
            }
            selected_group.set(Some(group_index));
            emit_selection(&handlers);
        });
        origins.append(&button);
    }
    origins
}

/// Builds the per-row/card "Atualizar" button shown for the Updates tab
/// (show_results' group_by_repository branch). Resolves the package to
/// update from package_groups at click time — not a captured clone — so a
/// pill switch after the row is built still updates whichever origin is
/// selected then.
fn update_button(
    group_index: usize,
    package_groups: &Rc<RefCell<Vec<PackageGroup>>>,
    handlers: &UpdatePackageHandlers,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(gettext("Atualizar"))
        .valign(gtk::Align::Center)
        .css_classes(["suggested-action"])
        .build();
    let package_groups = package_groups.clone();
    let handlers = handlers.clone();
    button.connect_clicked(move |_| {
        let package = package_groups
            .borrow()
            .get(group_index)
            .and_then(PackageGroup::selected)
            .cloned();
        let Some(package) = package else {
            return;
        };
        for handler in handlers.borrow().iter() {
            handler(package.clone());
        }
    });
    button
}

/// Builds the per-row/card "add to install queue" toggle shown for
/// not-yet-installed Search results (show_results' non-group_by_repository,
/// non-installed branch) — lets the user queue several packages across
/// different searches before installing them all at once via the top
/// "Instalar" button.
fn install_queue_toggle(
    group_index: usize,
    package_groups: &Rc<RefCell<Vec<PackageGroup>>>,
    queue: &Rc<RefCell<Vec<PackageRef>>>,
    queue_button: &gtk::Button,
    initially_queued: bool,
) -> gtk::ToggleButton {
    let toggle = gtk::ToggleButton::builder()
        .icon_name(if initially_queued {
            "object-select-symbolic"
        } else {
            "list-add-symbolic"
        })
        .tooltip_text(gettext("Adicionar à fila de instalação"))
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .active(initially_queued)
        .build();
    let package_groups = package_groups.clone();
    let queue = queue.clone();
    let queue_button = queue_button.clone();
    toggle.connect_toggled(move |button| {
        let package = package_groups
            .borrow()
            .get(group_index)
            .and_then(PackageGroup::selected)
            .cloned();
        let Some(package) = package else {
            return;
        };
        button.set_icon_name(if button.is_active() {
            "object-select-symbolic"
        } else {
            "list-add-symbolic"
        });
        let mut queue = queue.borrow_mut();
        if button.is_active() {
            if !queue
                .iter()
                .any(|queued| queued.origin == package.origin && queued.id == package.id)
            {
                queue.push(package);
            }
        } else {
            queue.retain(|queued| !(queued.origin == package.origin && queued.id == package.id));
        }
        update_install_queue_button(&queue_button, queue.len());
    });
    toggle
}

/// Refreshes the top "Instalar" button's label/visibility to match how many
/// packages are currently queued — hidden entirely at zero, matching the
/// rest of the tab-scoped top-bar actions (e.g. global_action).
fn update_install_queue_button(button: &gtk::Button, count: usize) {
    button.set_visible(count > 0);
    button.set_label(&gettext("Instalar ({count})").replace("{count}", &count.to_string()));
}

fn tab_button(label: &str) -> gtk::ToggleButton {
    gtk::ToggleButton::builder()
        .label(label)
        .css_classes(["flat", "module-tab"])
        .build()
}

fn value_or_dash(value: &str) -> &str {
    if value.is_empty() { "—" } else { value }
}

fn origin_rank(origin: &str) -> u8 {
    match origin {
        "official" => 0,
        "flathub" => 1,
        _ => 2,
    }
}

fn origin_label(origin: &str) -> String {
    match origin {
        "official" => gettext("Oficial"),
        "flathub" => "Flathub".to_string(),
        value => value.to_string(),
    }
}

/// Wire-value ("download"/"install") to display label for a per-package
/// progress bar's text — see SoftwarePackageProgress::phase.
fn phase_label(phase: &str) -> String {
    if phase == "download" {
        gettext("Baixando")
    } else {
        gettext("Instalando")
    }
}

/// Presents a one-button alert with `message` — the shared "something
/// failed" path for every software transaction, so the user (who only
/// knows Vega, not whatever package manager runs underneath it) always
/// gets a visible dialog instead of a status label they might not notice.
fn present_error_alert(parent: &impl IsA<gtk::Widget>, message: &str) {
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Não foi possível concluir a operação")),
        Some(message),
    );
    dialog.add_responses(&[("ok", &gettext("OK"))]);
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("ok");
    dialog.present(Some(parent));
}

fn group_packages(packages: Vec<PackageRef>) -> Vec<PackageGroup> {
    let mut grouped = BTreeMap::<String, Vec<PackageRef>>::new();
    for package in packages {
        let key = package.name.trim().to_lowercase();
        grouped.entry(key).or_default().push(package);
    }
    grouped
        .into_values()
        .map(|mut packages| {
            packages.sort_by_key(|package| origin_rank(&package.origin));
            PackageGroup {
                packages,
                selected: 0,
            }
        })
        .collect()
}

/// Same de-dup-by-name grouping as group_packages, but preserves the
/// backend's own ordering instead of re-sorting alphabetically — used for
/// the Updates tab, whose packages already arrive grouped/ordered by
/// source repository (see zypperGroupedUpdates on the Go side), an order
/// that must survive into the section headers show_results draws.
fn group_packages_preserving_order(packages: Vec<PackageRef>) -> Vec<PackageGroup> {
    let mut order = Vec::new();
    let mut grouped = HashMap::<String, Vec<PackageRef>>::new();
    for package in packages {
        let key = package.name.trim().to_lowercase();
        if !grouped.contains_key(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(package);
    }
    order
        .into_iter()
        .filter_map(|key| grouped.remove(&key))
        .map(|mut packages| {
            packages.sort_by_key(|package| origin_rank(&package.origin));
            PackageGroup {
                packages,
                selected: 0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(origin: &str, id: &str) -> PackageRef {
        PackageRef {
            origin: origin.into(),
            id: id.into(),
            name: "Example".into(),
            description: String::new(),
            installed: false,
            icon: String::new(),
            repository: String::new(),
        }
    }

    #[test]
    fn grouping_preserves_origins_and_prefers_official() {
        let result = group_packages(vec![
            package("flathub", "org.example.App"),
            package("official", "example"),
        ]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].packages.len(), 2);
        assert_eq!(result[0].selected().unwrap().origin, "official");
        assert_eq!(result[0].packages[1].origin, "flathub");
    }

    #[test]
    fn grouping_is_case_insensitive_but_keeps_distinct_apps() {
        let mut second = package("official", "another");
        second.name = "Another".into();
        let mut duplicate = package("flathub", "example-flatpak");
        duplicate.name = "example".into();
        let result = group_packages(vec![package("official", "example"), duplicate, second]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].packages.len(), 2);
    }

    #[test]
    fn preserving_order_grouping_keeps_arrival_order_not_alphabetical() {
        let mut zeta = package("official", "zeta");
        zeta.name = "Zeta".into();
        let mut alpha = package("official", "alpha");
        alpha.name = "Alpha".into();
        let result = group_packages_preserving_order(vec![zeta, alpha]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].selected().unwrap().name, "Zeta");
        assert_eq!(result[1].selected().unwrap().name, "Alpha");
    }

    #[test]
    fn package_text_is_safe_for_action_row_markup() {
        let escaped =
            gtk::glib::markup_escape_text("Devices & control points <maintainer@example.org>");
        assert_eq!(
            escaped,
            "Devices &amp; control points &lt;maintainer@example.org&gt;"
        );
    }
}
