use std::{cell::RefCell, rc::Rc};

use crate::i18n::gettext;
use adw::prelude::*;
use lyra_vega_dbus::{
    FirewallPort, FirewallService, FirewallStatus, NetworkInterface, ProxyConfig, WifiNetwork,
};

type WifiActionHandler = Rc<dyn Fn(WifiNetwork)>;

#[derive(Clone)]
pub struct NetworkPage {
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub interfaces: gtk::ListBox,
    pub interface_action: gtk::Button,
    pub wifi: gtk::ListBox,
    pub proxy: gtk::Label,
    pub proxy_http: gtk::Entry,
    pub proxy_https: gtk::Entry,
    pub proxy_socks: gtk::Entry,
    pub proxy_exceptions: gtk::Entry,
    pub proxy_apply: gtk::Button,
    pub vpn_status: gtk::Label,
    pub vpn_import: gtk::Button,
    pub firewall_status: gtk::Label,
    pub firewall_services: gtk::ListBox,
    pub firewall_action: gtk::Button,
    pub firewall_ports: gtk::ListBox,
    pub firewall_port_remove: gtk::Button,
    pub firewall_port_entry: gtk::Entry,
    pub firewall_protocol: gtk::DropDown,
    pub firewall_port_add: gtk::Button,
    interface_items: Rc<RefCell<Vec<NetworkInterface>>>,
    wifi_action_handlers: Rc<RefCell<Vec<WifiActionHandler>>>,
    firewall_items: Rc<RefCell<Vec<FirewallService>>>,
    firewall_port_items: Rc<RefCell<Vec<FirewallPort>>>,
}
impl NetworkPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando rede…"))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();
        let interfaces = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        interfaces.add_css_class("network-interfaces");
        let interface_action = gtk::Button::builder()
            .label(gettext("Configurar IPv4"))
            .halign(gtk::Align::Start)
            .sensitive(false)
            .build();
        let wifi = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        wifi.add_css_class("wifi-networks");
        let proxy = gtk::Label::builder()
            .label(gettext("Carregando configuração…"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let proxy_http = proxy_entry(&gettext("http://servidor:porta"));
        let proxy_https = proxy_entry(&gettext("http://servidor:porta"));
        let proxy_socks = proxy_entry(&gettext("socks://servidor:porta"));
        let proxy_exceptions = proxy_entry(&gettext("localhost, 127.0.0.1, domínio.local"));
        let proxy_grid = gtk::Grid::builder()
            .column_spacing(10)
            .row_spacing(8)
            .hexpand(true)
            .build();
        for (row, (label, entry)) in [
            ("HTTP".to_string(), &proxy_http),
            ("HTTPS".to_string(), &proxy_https),
            ("SOCKS".to_string(), &proxy_socks),
            (gettext("Exceções"), &proxy_exceptions),
        ]
        .into_iter()
        .enumerate()
        {
            proxy_grid.attach(
                &gtk::Label::builder().label(&label).xalign(0.0).build(),
                0,
                row as i32,
                1,
                1,
            );
            proxy_grid.attach(entry, 1, row as i32, 1, 1);
        }
        let proxy_apply = gtk::Button::builder()
            .label(gettext("Aplicar proxy"))
            .halign(gtk::Align::Start)
            .build();
        let vpn_status = gtk::Label::builder()
            .label(gettext(
                "Importe um perfil OpenVPN fornecido pelo seu serviço de VPN.",
            ))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let vpn_import = gtk::Button::builder()
            .label(gettext("Importar perfil .ovpn"))
            .halign(gtk::Align::Start)
            .build();
        let firewall_status = gtk::Label::builder()
            .label(gettext("Detectando firewall…"))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();
        let firewall_services = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        firewall_services.add_css_class("firewall-services");
        let firewall_action = gtk::Button::builder()
            .label(gettext("Permitir serviço"))
            .halign(gtk::Align::Start)
            .sensitive(false)
            .build();
        let firewall_ports = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        firewall_ports.add_css_class("firewall-ports");
        let firewall_port_remove = gtk::Button::builder()
            .label(gettext("Remover regra"))
            .halign(gtk::Align::Start)
            .sensitive(false)
            .build();
        let firewall_port_entry = gtk::Entry::builder()
            .placeholder_text(gettext("porta ou intervalo, ex.: 8080 ou 9000-9010"))
            .hexpand(true)
            .build();
        let firewall_protocol = gtk::DropDown::from_strings(&["tcp", "udp"]);
        let firewall_port_add = gtk::Button::builder()
            .label(gettext("Adicionar regra"))
            .css_classes(["suggested-action"])
            .build();
        let firewall_port_form = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        firewall_port_form.append(&firewall_port_entry);
        firewall_port_form.append(&firewall_protocol);
        firewall_port_form.append(&firewall_port_add);
        let interfaces_tab_content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        interfaces_tab_content.append(&status);
        interfaces_tab_content.append(&interfaces);
        interfaces_tab_content.append(&interface_action);

        let wifi_tab_content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        wifi_tab_content.append(&wifi);

        let proxy_tab_content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        proxy_tab_content.append(&proxy);
        proxy_tab_content.append(&proxy_grid);
        proxy_tab_content.append(&proxy_apply);

        let vpn_tab_content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        vpn_tab_content.append(&vpn_status);
        vpn_tab_content.append(&vpn_import);

        let firewall_tab_content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        firewall_tab_content.append(&firewall_status);
        firewall_tab_content.append(&firewall_services);
        firewall_tab_content.append(&firewall_action);
        firewall_tab_content.append(
            &gtk::Label::builder()
                .label(gettext("Regras de porta personalizadas"))
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        firewall_tab_content.append(&firewall_ports);
        firewall_tab_content.append(&firewall_port_remove);
        firewall_tab_content.append(&firewall_port_form);

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabs.add_css_class("module-tabs");
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .build();
        let mut group: Option<gtk::ToggleButton> = None;
        for (name, label, widget) in [
            ("interfaces", gettext("Interfaces"), &interfaces_tab_content),
            ("wifi", gettext("Wi‑Fi"), &wifi_tab_content),
            ("proxy", gettext("Proxy"), &proxy_tab_content),
            ("vpn", "VPN".to_string(), &vpn_tab_content),
            ("firewall", gettext("Firewall"), &firewall_tab_content),
        ] {
            stack.add_named(widget, Some(name));
            let button = tab_button(&label);
            if let Some(first) = &group {
                button.set_group(Some(first));
            } else {
                button.set_active(true);
                group = Some(button.clone());
            }
            let target_stack = stack.clone();
            button.connect_clicked(move |button| {
                if button.is_active() {
                    target_stack.set_visible_child_name(name);
                }
            });
            tabs.append(&button);
        }

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.add_css_class("compact-page");
        content.append(
            &gtk::Label::builder()
                .label(gettext("Rede e Firewall"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(
            &gtk::Label::builder()
                .label(gettext("Interfaces, Wi‑Fi, proxy e proteção da rede"))
                .xalign(0.0)
                .css_classes(["dim-label"])
                .build(),
        );
        content.append(&tabs);
        content.append(&stack);
        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        let page = Self {
            root,
            status,
            interfaces,
            interface_action,
            wifi,
            proxy,
            proxy_http,
            proxy_https,
            proxy_socks,
            proxy_exceptions,
            proxy_apply,
            vpn_status,
            vpn_import,
            firewall_status,
            firewall_services,
            firewall_action,
            firewall_ports,
            firewall_port_remove,
            firewall_port_entry,
            firewall_protocol,
            firewall_port_add,
            interface_items: Rc::new(RefCell::new(Vec::new())),
            wifi_action_handlers: Rc::new(RefCell::new(Vec::new())),
            firewall_items: Rc::new(RefCell::new(Vec::new())),
            firewall_port_items: Rc::new(RefCell::new(Vec::new())),
        };
        let interface_page = page.clone();
        page.interfaces
            .connect_row_selected(move |_, _| interface_page.update_interface_action());
        let selection_page = page.clone();
        page.firewall_services
            .connect_row_selected(move |_, _| selection_page.update_firewall_action());
        let port_selection_page = page.clone();
        page.firewall_ports
            .connect_row_selected(move |_, _| port_selection_page.update_firewall_port_remove());
        page
    }
    pub fn show_interfaces(&self, items: &[NetworkInterface]) {
        clear(&self.interfaces);
        for i in items {
            let title = format!("{} • {}", i.name, i.device);
            let sub = format!(
                "{} • {} • IPv4 {} • {} • {}",
                i.kind,
                i.state,
                dash(&i.ipv4),
                dash(&i.speed),
                i.mac
            );
            self.interfaces.append(
                &adw::ActionRow::builder()
                    .title(gtk::glib::markup_escape_text(&title))
                    .subtitle(gtk::glib::markup_escape_text(&sub))
                    .build(),
            );
        }
        if items.is_empty() {
            empty(&self.interfaces, &gettext("Nenhuma interface detectada"));
        }
        *self.interface_items.borrow_mut() = items.to_vec();
        self.update_interface_action();
    }
    pub fn show_wifi(&self, items: &[WifiNetwork]) {
        clear(&self.wifi);
        for i in items {
            let sub = gettext("{security} • sinal {signal}% • {state}")
                .replace("{security}", dash(&i.security))
                .replace("{signal}", &i.signal.to_string())
                .replace(
                    "{state}",
                    &if i.active {
                        gettext("Conectada")
                    } else {
                        gettext("Disponível")
                    },
                );
            let row = adw::ActionRow::builder()
                .title(gtk::glib::markup_escape_text(&i.ssid))
                .subtitle(gtk::glib::markup_escape_text(&sub))
                .build();
            let button = gtk::Button::with_label(&if i.active {
                gettext("Desconectar")
            } else {
                gettext("Conectar")
            });
            button.add_css_class("wifi-row-action");
            button.set_valign(gtk::Align::Center);
            if !i.active {
                button.add_css_class("suggested-action");
            }
            let network = i.clone();
            let handlers = self.wifi_action_handlers.clone();
            button.connect_clicked(move |_| {
                for handler in handlers.borrow().iter() {
                    handler(network.clone());
                }
            });
            row.add_suffix(&button);
            self.wifi.append(&row);
        }
        if items.is_empty() {
            empty(&self.wifi, &gettext("Nenhuma rede Wi‑Fi detectada"));
        }
    }
    pub fn show_proxy(&self, p: &ProxyConfig) {
        self.proxy_http.set_text(&p.http);
        self.proxy_https.set_text(&p.https);
        self.proxy_socks.set_text(&p.socks);
        self.proxy_exceptions.set_text(&p.no_proxy);
        self.proxy.set_label(&if p.http.is_empty()
            && p.https.is_empty()
            && p.socks.is_empty()
            && p.no_proxy.is_empty()
        {
            gettext("Proxy não configurado")
        } else {
            gettext("Configuração carregada de /etc/environment")
        });
    }

    pub fn proxy_config(&self) -> ProxyConfig {
        ProxyConfig {
            http: self.proxy_http.text().trim().to_owned(),
            https: self.proxy_https.text().trim().to_owned(),
            socks: self.proxy_socks.text().trim().to_owned(),
            no_proxy: self.proxy_exceptions.text().trim().to_owned(),
        }
    }

    pub fn show_firewall(&self, status: &FirewallStatus, services: &[FirewallService]) {
        self.firewall_status.set_label(
            &gettext("{state} • zona/perfil: {zone}")
                .replace(
                    "{state}",
                    &if status.enabled {
                        gettext("Ativo")
                    } else {
                        gettext("Inativo")
                    },
                )
                .replace("{zone}", dash(&status.active_zone)),
        );
        clear(&self.firewall_services);
        for service in services {
            let row = adw::ActionRow::builder()
                .title(gtk::glib::markup_escape_text(&service.label))
                .subtitle(gtk::glib::markup_escape_text(&service.name))
                .title_lines(1)
                .subtitle_lines(1)
                .activatable(true)
                .build();
            row.add_suffix(
                &gtk::Label::builder()
                    .label(if service.enabled {
                        gettext("Permitido")
                    } else {
                        gettext("Bloqueado")
                    })
                    .valign(gtk::Align::Center)
                    .css_classes(if service.enabled {
                        vec!["success"]
                    } else {
                        vec!["dim-label"]
                    })
                    .build(),
            );
            self.firewall_services.append(&row);
        }
        if services.is_empty() {
            empty(
                &self.firewall_services,
                &gettext("Nenhum serviço publicado"),
            );
        }
        *self.firewall_items.borrow_mut() = services.to_vec();
        self.update_firewall_action();
    }

    pub fn selected_firewall_service(&self) -> Option<FirewallService> {
        let index = self.firewall_services.selected_row()?.index() as usize;
        self.firewall_items.borrow().get(index).cloned()
    }

    pub fn show_firewall_ports(&self, ports: &[FirewallPort]) {
        clear(&self.firewall_ports);
        for port in ports {
            let row = adw::ActionRow::builder()
                .title(gtk::glib::markup_escape_text(&format!(
                    "{}/{}",
                    port.port, port.protocol
                )))
                .build();
            self.firewall_ports.append(&row);
        }
        if ports.is_empty() {
            empty(
                &self.firewall_ports,
                &gettext("Nenhuma regra de porta personalizada"),
            );
        }
        *self.firewall_port_items.borrow_mut() = ports.to_vec();
        self.update_firewall_port_remove();
    }

    pub fn selected_firewall_port(&self) -> Option<FirewallPort> {
        let index = self.firewall_ports.selected_row()?.index() as usize;
        self.firewall_port_items.borrow().get(index).cloned()
    }

    /// (porta, protocolo) atualmente digitados no formulário de nova regra.
    pub fn firewall_port_input(&self) -> (String, String) {
        let port = self.firewall_port_entry.text().trim().to_owned();
        let protocol = self
            .firewall_protocol
            .selected_item()
            .and_downcast::<gtk::StringObject>()
            .map(|item| item.string().to_string())
            .unwrap_or_else(|| "tcp".to_owned());
        (port, protocol)
    }

    pub fn clear_firewall_port_entry(&self) {
        self.firewall_port_entry.set_text("");
    }

    fn update_firewall_port_remove(&self) {
        self.firewall_port_remove
            .set_sensitive(self.selected_firewall_port().is_some());
    }

    pub fn connect_wifi_action(&self, handler: impl Fn(WifiNetwork) + 'static) {
        self.wifi_action_handlers
            .borrow_mut()
            .push(Rc::new(handler));
    }

    pub fn selected_interface(&self) -> Option<NetworkInterface> {
        let index = self.interfaces.selected_row()?.index() as usize;
        self.interface_items.borrow().get(index).cloned()
    }

    fn update_interface_action(&self) {
        self.interface_action
            .set_sensitive(self.selected_interface().is_some());
    }

    fn update_firewall_action(&self) {
        let Some(service) = self.selected_firewall_service() else {
            self.firewall_action.set_sensitive(false);
            return;
        };
        self.firewall_action.set_sensitive(true);
        self.firewall_action.set_label(&if service.enabled {
            gettext("Bloquear serviço")
        } else {
            gettext("Permitir serviço")
        });
        if service.enabled {
            self.firewall_action.remove_css_class("suggested-action");
        } else {
            self.firewall_action.add_css_class("suggested-action");
        }
    }
}
fn tab_button(label: &str) -> gtk::ToggleButton {
    gtk::ToggleButton::builder()
        .label(label)
        .css_classes(["flat", "module-tab"])
        .build()
}
fn clear(l: &gtk::ListBox) {
    while let Some(c) = l.first_child() {
        l.remove(&c);
    }
}
fn empty(l: &gtk::ListBox, t: &str) {
    l.append(&adw::ActionRow::builder().title(t).build());
}
fn dash(v: &str) -> &str {
    if v.is_empty() { "—" } else { v }
}

fn proxy_entry(placeholder: &str) -> gtk::Entry {
    gtk::Entry::builder()
        .placeholder_text(placeholder)
        .hexpand(true)
        .build()
}
