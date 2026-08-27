use std::{cell::RefCell, rc::Rc};

use crate::i18n::gettext;
use adw::prelude::*;

use lyra_vega_dbus::{BluetoothDevice, BluetoothStatus};

#[derive(Clone)]
pub struct BluetoothPage {
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub adapter: gtk::ListBox,
    pub devices: gtk::ListBox,
    pub power: gtk::Button,
    pub scan: gtk::Button,
    pub device_action: gtk::Button,
    pub send_file: gtk::Button,
    pub receive_files: gtk::Button,
    current_status: Rc<RefCell<Option<BluetoothStatus>>>,
    device_items: Rc<RefCell<Vec<BluetoothDevice>>>,
}

impl BluetoothPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Detectando Bluetooth…"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let adapter = list();
        let devices = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        devices.add_css_class("bluetooth-devices");
        let power = gtk::Button::builder()
            .label(gettext("Ligar Bluetooth"))
            .sensitive(false)
            .build();
        let scan = gtk::Button::builder()
            .label(gettext("Buscar dispositivos"))
            .sensitive(false)
            .build();
        let adapter_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        adapter_actions.append(&power);
        adapter_actions.append(&scan);
        let device_action = gtk::Button::builder()
            .label(gettext("Parear"))
            .halign(gtk::Align::Start)
            .sensitive(false)
            .build();
        let send_file = gtk::Button::builder()
            .label(gettext("Enviar arquivo"))
            .sensitive(false)
            .build();
        let receive_files = gtk::Button::builder()
            .label(gettext("Receber arquivos"))
            .sensitive(false)
            .build();
        let device_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        device_actions.append(&device_action);
        device_actions.append(&send_file);
        device_actions.append(&receive_files);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.add_css_class("compact-page");
        content.append(
            &gtk::Label::builder()
                .label(gettext("Bluetooth"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(
            &gtk::Label::builder()
                .label(gettext(
                    "Adaptador, dispositivos e transferência de arquivos",
                ))
                .xalign(0.0)
                .css_classes(["dim-label"])
                .build(),
        );
        content.append(&status);
        content.append(&section(&gettext("Adaptador"), &adapter));
        content.append(&adapter_actions);
        content.append(&section(&gettext("Dispositivos"), &devices));
        content.append(&device_actions);
        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        let page = Self {
            root,
            status,
            adapter,
            devices,
            power,
            scan,
            device_action,
            send_file,
            receive_files,
            current_status: Rc::new(RefCell::new(None)),
            device_items: Rc::new(RefCell::new(Vec::new())),
        };
        let selection_page = page.clone();
        page.devices
            .connect_row_selected(move |_, _| selection_page.update_device_action());
        page
    }

    pub fn show(&self, status: &BluetoothStatus, devices: &[BluetoothDevice]) {
        clear(&self.adapter);
        clear(&self.devices);
        if !status.available {
            self.status
                .set_label(&gettext("Bluetooth não está disponível nesta máquina."));
            empty(&self.adapter, &gettext("bluetoothctl não detectado"));
            empty(&self.devices, &gettext("Nenhum dispositivo disponível"));
            self.power.set_sensitive(false);
            self.scan.set_sensitive(false);
            self.device_action.set_sensitive(false);
            self.send_file.set_sensitive(false);
            self.receive_files.set_sensitive(false);
            return;
        }
        self.status.set_label(&if status.powered {
            gettext("Bluetooth ligado")
        } else {
            gettext("Bluetooth desligado")
        });
        self.adapter.append(&row(
            &display_value(&status.controller_name, &status.controller),
            &format!(
                "{} • {} • {}",
                state(&gettext("Descoberta"), status.discoverable),
                state(&gettext("Pareamento"), status.pairable),
                if status.scanning {
                    gettext("Buscando dispositivos")
                } else {
                    gettext("Busca parada")
                }
            ),
        ));
        self.adapter.append(&row(
            &gettext("Transferência de arquivos"),
            &if status.transfer_available {
                if status.receiver_active {
                    gettext("Recebimento ativo")
                } else {
                    gettext("Disponível")
                }
            } else {
                gettext("bt-obex não instalado")
            },
        ));
        for device in devices {
            let connection = if device.connected {
                gettext("Conectado")
            } else if device.paired {
                gettext("Pareado")
            } else {
                gettext("Disponível")
            };
            let details = gettext("{address} • {connection} • {trust} • sinal {rssi} dBm")
                .replace("{address}", &device.address)
                .replace("{connection}", &connection)
                .replace(
                    "{trust}",
                    &if device.trusted {
                        gettext("Confiável")
                    } else {
                        gettext("Não confiável")
                    },
                )
                .replace("{rssi}", &device.rssi.to_string());
            let item = row(device.display_name(), &details);
            item.add_prefix(
                &gtk::Image::builder()
                    .icon_name(device_icon(&device.icon))
                    .pixel_size(20)
                    .build(),
            );
            self.devices.append(&item);
        }
        if devices.is_empty() {
            empty(
                &self.devices,
                &gettext("Nenhum dispositivo conhecido ou encontrado"),
            );
        }
        *self.current_status.borrow_mut() = Some(status.clone());
        *self.device_items.borrow_mut() = devices.to_vec();
        self.power.set_sensitive(true);
        self.power.set_label(&if status.powered {
            gettext("Desligar Bluetooth")
        } else {
            gettext("Ligar Bluetooth")
        });
        self.scan.set_sensitive(status.powered);
        self.scan.set_label(&if status.scanning {
            gettext("Parar busca")
        } else {
            gettext("Buscar dispositivos")
        });
        self.receive_files
            .set_sensitive(status.transfer_available && !status.receiver_active);
        self.receive_files.set_label(&if status.receiver_active {
            gettext("Recebimento ativo")
        } else {
            gettext("Receber arquivos")
        });
        self.update_device_action();
    }

    pub fn current_status(&self) -> Option<BluetoothStatus> {
        self.current_status.borrow().clone()
    }

    pub fn selected_device(&self) -> Option<BluetoothDevice> {
        let index = self.devices.selected_row()?.index() as usize;
        self.device_items.borrow().get(index).cloned()
    }

    fn update_device_action(&self) {
        let Some(device) = self.selected_device() else {
            self.device_action.set_sensitive(false);
            self.send_file.set_sensitive(false);
            return;
        };
        self.device_action.set_sensitive(true);
        self.device_action.set_label(&if !device.paired {
            gettext("Parear")
        } else if device.connected {
            gettext("Desconectar")
        } else {
            gettext("Conectar")
        });
        self.send_file.set_sensitive(
            device.paired
                && self
                    .current_status()
                    .is_some_and(|status| status.transfer_available),
        );
    }
}

fn list() -> gtk::ListBox {
    gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build()
}

fn section(title: &str, list: &gtk::ListBox) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    section.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["title-2"])
            .build(),
    );
    section.append(list);
    section
}

fn row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(gtk::glib::markup_escape_text(title))
        .subtitle(gtk::glib::markup_escape_text(subtitle))
        .title_lines(1)
        .subtitle_lines(1)
        .build()
}

fn clear(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn empty(list: &gtk::ListBox, message: &str) {
    list.append(&adw::ActionRow::builder().title(message).build());
}

fn state(label: &str, enabled: bool) -> String {
    format!(
        "{label} {}",
        if enabled {
            gettext("ativa")
        } else {
            gettext("inativa")
        }
    )
}

fn display_value(preferred: &str, fallback: &str) -> String {
    if preferred.is_empty() {
        if fallback.is_empty() {
            gettext("Adaptador")
        } else {
            fallback.to_string()
        }
    } else {
        preferred.to_string()
    }
}

fn device_icon(icon: &str) -> &str {
    match icon {
        "audio-card" | "audio-headphones" | "audio-headset" => "audio-headphones-symbolic",
        "input-keyboard" => "input-keyboard-symbolic",
        "input-mouse" => "input-mouse-symbolic",
        "phone" => "phone-symbolic",
        _ => "bluetooth-symbolic",
    }
}
