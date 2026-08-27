use crate::assistant::{Attachment, Message, Provider, Settings};
use crate::i18n::gettext;
use adw::prelude::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub struct AssistantPage {
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub transcript: gtk::TextView,
    pub prompt: gtk::TextView,
    pub attach: gtk::Button,
    pub send: gtk::Button,
    pub clear_history: gtk::Button,
    pub provider: gtk::DropDown,
    pub model: gtk::DropDown,
    pub refresh_models: gtk::Button,
    pub daily_limit: gtk::SpinButton,
    pub max_rounds: gtk::SpinButton,
    pub api_key: gtk::PasswordEntry,
    pub save_settings: gtk::Button,
    pub save_key: gtk::Button,
    pub remove_key: gtk::Button,
    attachments_flow: gtk::FlowBox,
    history: Rc<RefCell<Vec<Message>>>,
    models: Rc<RefCell<Vec<String>>>,
    pending_attachments: Rc<RefCell<Vec<Attachment>>>,
}

impl AssistantPage {
    pub fn new(settings: &Settings, history: Vec<Message>) -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Pronto"))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let transcript = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(16)
            .right_margin(16)
            .top_margin(16)
            .bottom_margin(16)
            .vexpand(true)
            .build();
        transcript.add_css_class("assistant-transcript");
        let transcript_scroll = gtk::ScrolledWindow::builder()
            .child(&transcript)
            .min_content_height(380)
            .vexpand(true)
            .build();
        transcript_scroll.add_css_class("card");
        // TextView em vez de Entry porque a mensagem pode ter várias linhas
        // (colar um trecho de log, por exemplo); Enter sozinho envia, e
        // Shift+Enter quebra linha — ver o EventControllerKey logo abaixo.
        let prompt = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .accepts_tab(false)
            .top_margin(6)
            .bottom_margin(6)
            .left_margin(8)
            .right_margin(8)
            .hexpand(true)
            .build();
        let prompt_scroll = gtk::ScrolledWindow::builder()
            .child(&prompt)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(36)
            .max_content_height(120)
            .propagate_natural_height(true)
            .hexpand(true)
            .build();
        prompt_scroll.add_css_class("card");
        let attach = gtk::Button::builder()
            .icon_name("mail-attachment-symbolic")
            .tooltip_text(gettext("Anexar arquivo ou imagem"))
            .valign(gtk::Align::End)
            .build();
        let send = gtk::Button::builder()
            .label(gettext("Enviar"))
            .css_classes(["suggested-action"])
            .valign(gtk::Align::End)
            .build();
        let clear_history = gtk::Button::builder()
            .label(gettext("Limpar conversa"))
            .valign(gtk::Align::End)
            .build();
        let send_on_enter = send.clone();
        let enter_controller = gtk::EventControllerKey::new();
        enter_controller.connect_key_pressed(move |_, key, _, modifiers| {
            if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter)
                && !modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK)
            {
                send_on_enter.emit_clicked();
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        });
        prompt.add_controller(enter_controller);
        let attachments_flow = gtk::FlowBox::builder()
            .column_spacing(6)
            .row_spacing(6)
            .min_children_per_line(1)
            .max_children_per_line(8)
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(false)
            .visible(false)
            .build();
        let input_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        input_row.append(&prompt_scroll);
        input_row.append(&attach);
        input_row.append(&send);
        input_row.append(&clear_history);
        let composer = gtk::Box::new(gtk::Orientation::Vertical, 8);
        composer.append(&attachments_flow);
        composer.append(&input_row);
        let provider = gtk::DropDown::from_strings(&Provider::ALL.map(Provider::label));
        provider.set_selected(settings.provider.index());
        let model = gtk::DropDown::from_strings(&[settings.model()]);
        model.set_hexpand(true);
        let refresh_models = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(gettext("Atualizar lista de modelos"))
            .build();
        let model_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        model_box.append(&model);
        model_box.append(&refresh_models);
        let daily_limit = gtk::SpinButton::with_range(1.0, 5000.0, 1.0);
        daily_limit.set_value(f64::from(settings.max_messages_per_day));
        let max_rounds = gtk::SpinButton::with_range(1.0, 20.0, 1.0);
        max_rounds.set_value(f64::from(settings.max_rounds_per_message));
        let api_key = gtk::PasswordEntry::builder()
            .placeholder_text(gettext("Chave armazenada somente no keyring"))
            .show_peek_icon(true)
            .hexpand(true)
            .build();
        let save_settings = gtk::Button::builder()
            .label(gettext("Salvar configurações"))
            .css_classes(["suggested-action"])
            .build();
        let save_key = gtk::Button::with_label(&gettext("Salvar chave"));
        let remove_key = gtk::Button::builder()
            .label(gettext("Remover chave"))
            .css_classes(["destructive-action"])
            .build();
        let settings_group = adw::PreferencesGroup::builder()
            .title(gettext("Configurações e credenciais"))
            .description(gettext(
                "As chaves ficam no Secret Service e nunca em arquivos do Lyra Vega",
            ))
            .build();
        settings_group.add(&row(&gettext("Provedor"), &provider));
        settings_group.add(&row(&gettext("Modelo"), &model_box));
        settings_group.add(&row(&gettext("Limite diário"), &daily_limit));
        settings_group.add(&row(&gettext("Máximo de etapas"), &max_rounds));
        settings_group.add(&row(&gettext("Chave de API"), &api_key));
        let settings_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        settings_actions.set_halign(gtk::Align::End);
        settings_actions.append(&save_settings);
        settings_actions.append(&save_key);
        settings_actions.append(&remove_key);
        let chat_tab = gtk::ToggleButton::builder()
            .label(gettext("Chat"))
            .css_classes(["flat", "module-tab"])
            .build();
        chat_tab.set_active(true);
        let settings_tab = gtk::ToggleButton::builder()
            .label(gettext("Configurações e Credenciais"))
            .css_classes(["flat", "module-tab"])
            .build();
        settings_tab.set_group(Some(&chat_tab));
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabs.add_css_class("module-tabs");
        tabs.append(&chat_tab);
        tabs.append(&settings_tab);

        let chat_content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        chat_content.append(&transcript_scroll);
        chat_content.append(&composer);

        let settings_content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        settings_content.append(&settings_group);
        settings_content.append(&settings_actions);

        let tab_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .build();
        tab_stack.add_named(&chat_content, Some("chat"));
        tab_stack.add_named(&settings_content, Some("settings"));

        let chat_stack = tab_stack.clone();
        chat_tab.connect_clicked(move |button| {
            if button.is_active() {
                chat_stack.set_visible_child_name("chat");
            }
        });
        let settings_stack = tab_stack.clone();
        settings_tab.connect_clicked(move |button| {
            if button.is_active() {
                settings_stack.set_visible_child_name("settings");
            }
        });

        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.add_css_class("content-page");
        content.append(
            &gtk::Label::builder()
                .label(gettext("Assistente de IA"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(
            &gtk::Label::builder()
                .label(gettext(
                    "Orientação contextual com privacidade, limites e aprovação explícita para alterações",
                ))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
        // Compartilhado entre as duas abas (fica acima do seletor de abas):
        // salvar configurações, salvar chave e remover chave mostram o
        // resultado aqui mesmo se o usuário estiver na aba Chat.
        content.append(&status);
        content.append(&tabs);
        content.append(&tab_stack);
        let root = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build()
            .upcast();
        let page = Self {
            root,
            status,
            transcript,
            prompt,
            attach,
            send,
            clear_history,
            provider,
            model,
            refresh_models,
            daily_limit,
            max_rounds,
            api_key,
            save_settings,
            save_key,
            remove_key,
            attachments_flow,
            history: Rc::new(RefCell::new(history)),
            models: Rc::new(RefCell::new(vec![settings.model().to_owned()])),
            pending_attachments: Rc::new(RefCell::new(Vec::new())),
        };
        page.render_history();
        page
    }
    pub fn settings(&self) -> Settings {
        let mut settings = crate::assistant::load_settings();
        settings.provider = Provider::from_index(self.provider.selected());
        settings.set_model(self.selected_model());
        settings.max_messages_per_day = self.daily_limit.value_as_int().clamp(1, 5000) as u32;
        settings.max_rounds_per_message = self.max_rounds.value_as_int().clamp(1, 20) as u32;
        settings
    }
    pub fn show_models(&self, mut models: Vec<String>, selected: &str) {
        if !models.iter().any(|model| model == selected) && !selected.is_empty() {
            models.insert(0, selected.to_owned());
        }
        let labels = models.iter().map(String::as_str).collect::<Vec<_>>();
        self.model.set_model(Some(&gtk::StringList::new(&labels)));
        let index = models
            .iter()
            .position(|model| model == selected)
            .unwrap_or(0);
        self.model.set_selected(index as u32);
        *self.models.borrow_mut() = models;
    }
    pub fn selected_model(&self) -> String {
        self.models
            .borrow()
            .get(self.model.selected() as usize)
            .cloned()
            .unwrap_or_default()
    }
    pub fn history(&self) -> Vec<Message> {
        self.history.borrow().clone()
    }
    pub fn prompt_text(&self) -> String {
        let buffer = self.prompt.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .trim()
            .to_owned()
    }
    pub fn clear_prompt(&self) {
        self.prompt.buffer().set_text("");
    }
    pub fn append(&self, role: &str, content: String, attachments: Vec<Attachment>) {
        self.history.borrow_mut().push(Message {
            role: role.into(),
            content,
            attachments,
        });
        self.render_history();
    }
    pub async fn append_progressively(&self, content: String) {
        self.history.borrow_mut().push(Message {
            role: "assistant".into(),
            content: String::new(),
            attachments: Vec::new(),
        });
        let words = content.split_whitespace().collect::<Vec<_>>();
        for end in (8..words.len())
            .step_by(8)
            .chain(std::iter::once(words.len()))
        {
            if let Some(message) = self.history.borrow_mut().last_mut() {
                message.content = words[..end].join(" ");
            }
            self.render_history();
            gtk::glib::timeout_future(std::time::Duration::from_millis(18)).await;
        }
        if words.is_empty() {
            if let Some(message) = self.history.borrow_mut().last_mut() {
                message.content = content;
            }
            self.render_history();
        }
    }
    pub fn clear(&self) {
        self.history.borrow_mut().clear();
        self.render_history();
    }
    pub fn set_busy(&self, busy: bool) {
        self.prompt.set_sensitive(!busy);
        self.send.set_sensitive(!busy);
        self.send.set_label(&if busy {
            gettext("Pensando…")
        } else {
            gettext("Enviar")
        });
    }
    pub fn render_history(&self) {
        let history = self.history.borrow();
        let text = if history.is_empty() {
            gettext(
                "Olá! Posso explicar o estado do sistema e ajudar com tarefas no Lyra Vega. Configure um provedor e sua chave abaixo para começar.",
            )
        } else {
            history
                .iter()
                .map(|message| {
                    let attachments = message
                        .attachments
                        .iter()
                        .map(|attachment| gettext("📎 {name}").replace("{name}", &attachment.name))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!(
                        "{}\n{}{}",
                        if message.role == "user" {
                            gettext("Você")
                        } else {
                            "Lyra Vega".to_string()
                        },
                        message.content,
                        if attachments.is_empty() {
                            String::new()
                        } else {
                            format!("\n{attachments}")
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        let buffer = self.transcript.buffer();
        buffer.set_text(&text);
        // Rola pro fim a cada atualização (inclusive as parciais do efeito
        // de "digitando" em append_progressively) pra acompanhar a resposta
        // chegando sem o usuário precisar arrastar a barra de rolagem.
        let mut end = buffer.end_iter();
        self.transcript
            .scroll_to_iter(&mut end, 0.0, false, 0.0, 0.0);
    }
    /// Anexos escolhidos pelo usuário mas ainda não enviados — mostrados
    /// como chips acima do campo de texto até o Enviar ou uma remoção.
    pub fn has_staged_attachments(&self) -> bool {
        !self.pending_attachments.borrow().is_empty()
    }
    pub fn stage_attachment(&self, attachment: Attachment) {
        self.pending_attachments.borrow_mut().push(attachment);
        self.render_attachments();
    }
    /// Esvazia os anexos pendentes e devolve para quem for montar a
    /// mensagem do usuário (chamado ao clicar em Enviar).
    pub fn take_staged_attachments(&self) -> Vec<Attachment> {
        let attachments = std::mem::take(&mut *self.pending_attachments.borrow_mut());
        self.render_attachments();
        attachments
    }
    fn render_attachments(&self) {
        while let Some(child) = self.attachments_flow.first_child() {
            self.attachments_flow.remove(&child);
        }
        let pending = self.pending_attachments.borrow();
        self.attachments_flow.set_visible(!pending.is_empty());
        for (index, attachment) in pending.iter().enumerate() {
            let chip = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            chip.add_css_class("card");
            chip.set_margin_top(2);
            chip.set_margin_bottom(2);
            chip.append(
                &gtk::Image::builder()
                    .icon_name(if attachment.is_image() {
                        "image-x-generic-symbolic"
                    } else {
                        "text-x-generic-symbolic"
                    })
                    .build(),
            );
            chip.append(
                &gtk::Label::builder()
                    .label(&attachment.name)
                    .ellipsize(gtk::pango::EllipsizeMode::Middle)
                    .max_width_chars(24)
                    .build(),
            );
            let remove = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .css_classes(["flat", "circular"])
                .tooltip_text(gettext("Remover anexo"))
                .build();
            let page = self.clone();
            remove.connect_clicked(move |_| {
                if index < page.pending_attachments.borrow().len() {
                    page.pending_attachments.borrow_mut().remove(index);
                }
                page.render_attachments();
            });
            chip.append(&remove);
            self.attachments_flow.insert(&chip, -1);
        }
    }
}
fn row(title: &str, widget: &impl IsA<gtk::Widget>) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).build();
    row.add_suffix(widget);
    row
}
