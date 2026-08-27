use std::{cell::RefCell, rc::Rc};

use crate::i18n::gettext;
use adw::prelude::*;

use super::sparkline::Sparkline;
use lyra_vega_dbus::{ProcessInfo, SystemMetrics};

type KillHandlers = Rc<RefCell<Vec<Rc<dyn Fn(ProcessInfo)>>>>;

const HISTORY_CAPACITY: usize = 60;

#[derive(Clone)]
pub struct MonitorPage {
    pub root: gtk::Widget,
    pub status: gtk::Label,
    pub processes_status: gtk::Label,
    pub cpu: gtk::Label,
    pub gpu: gtk::Label,
    pub memory: gtk::Label,
    pub swap: gtk::Label,
    pub disk: gtk::Label,
    pub network: gtk::Label,
    pub list: gtk::ListBox,
    cpu_graph: Sparkline,
    gpu_graph: Sparkline,
    memory_graph: Sparkline,
    swap_graph: Sparkline,
    disk_graph: Sparkline,
    network_graph: Sparkline,
    cpu_cores_flow: gtk::FlowBox,
    core_graphs: Rc<RefCell<Vec<MiniGraph>>>,
    gpu_devices_flow: gtk::FlowBox,
    gpu_device_graphs: Rc<RefCell<Vec<MiniGraph>>>,
    kill_handlers: KillHandlers,
}

/// One CPU core or GPU device card: numbered percentage plus its own history.
#[derive(Clone)]
struct MiniGraph {
    label: gtk::Label,
    graph: Sparkline,
}

impl MonitorPage {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .label(gettext("Carregando métricas…"))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();
        let processes_status = gtk::Label::builder()
            .label(gettext("Carregando processos…"))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();

        let cpu = value_label();
        let gpu = value_label();
        let memory = value_label();
        let swap = value_label();
        let disk = value_label();
        let network = value_label();
        // CPU/memória/swap são percentuais (0–100 fixo); disco/rede são
        // taxas sem teto natural, então a escala do gráfico se ajusta ao
        // pico recente da própria série.
        let cpu_graph = Sparkline::new(HISTORY_CAPACITY, Some(100.0));
        let gpu_graph = Sparkline::new(HISTORY_CAPACITY, Some(100.0));
        let memory_graph = Sparkline::new(HISTORY_CAPACITY, Some(100.0));
        let swap_graph = Sparkline::new(HISTORY_CAPACITY, Some(100.0));
        let disk_graph = Sparkline::new(HISTORY_CAPACITY, None);
        let network_graph = Sparkline::new(HISTORY_CAPACITY, None);

        // Quantidade de núcleos só é conhecida na primeira resposta do
        // vegad — os mini-gráficos de cada núcleo são criados sob demanda
        // em update_core_graphs, não aqui.
        let cpu_cores_flow = gtk::FlowBox::builder()
            .column_spacing(4)
            .row_spacing(4)
            .min_children_per_line(4)
            .max_children_per_line(16)
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .build();
        let gpu_devices_flow = gtk::FlowBox::builder()
            .column_spacing(4)
            .row_spacing(4)
            .min_children_per_line(4)
            .max_children_per_line(16)
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .build();

        let cpu_card = new_card(&gettext("CPU"));
        cpu_card.append(&cpu);
        cpu_card.append(&cpu_graph.widget);
        cpu_card.append(&cpu_cores_flow);

        let gpu_card = new_card(&gettext("GPU"));
        gpu_card.append(&gpu);
        gpu_card.append(&gpu_graph.widget);
        gpu_card.append(&gpu_devices_flow);

        let memory_card = new_card(&gettext("Memória"));
        memory_card.append(&memory);
        memory_card.append(&memory_graph.widget);

        let swap_card = new_card(&gettext("Swap"));
        swap_card.append(&swap);
        swap_card.append(&swap_graph.widget);

        let disk_card = new_card(&gettext("Disco"));
        disk_card.append(&disk);
        disk_card.append(&disk_graph.widget);

        let network_card = new_card(&gettext("Rede"));
        network_card.append(&network);
        network_card.append(&network_graph.widget);

        // CPU e GPU ficam empilhados; memória+swap e disco+rede seguem
        // emparelhados, dividindo a largura da linha. Altura fixa (não
        // vexpand): sem isso os cards esticavam até preencher qualquer
        // altura que o Stack desse à aba, inclusive por engano (ver
        // vhomogeneous acima).
        for card in [&cpu_card, &gpu_card] {
            card.set_size_request(-1, 220);
            card.set_hexpand(true);
        }
        for card in [&memory_card, &swap_card, &disk_card, &network_card] {
            card.set_size_request(-1, 160);
            card.set_hexpand(true);
        }
        let cpu_gpu_column = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .build();
        cpu_gpu_column.append(&cpu_card);
        cpu_gpu_column.append(&gpu_card);
        let memory_swap_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        memory_swap_row.append(&memory_card);
        memory_swap_row.append(&swap_card);
        let disk_network_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        disk_network_row.append(&disk_card);
        disk_network_row.append(&network_card);

        let metrics_flow = gtk::Box::new(gtk::Orientation::Vertical, 12);
        metrics_flow.append(&cpu_gpu_column);
        metrics_flow.append(&memory_swap_row);
        metrics_flow.append(&disk_network_row);

        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();

        let resources_tab_content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        resources_tab_content.append(&status);
        resources_tab_content.append(&metrics_flow);

        let processes_tab_content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        processes_tab_content.append(&processes_status);
        processes_tab_content.append(&list);

        let resources_tab = tab_button(&gettext("Recursos"));
        let processes_tab = tab_button(&gettext("Processos"));
        resources_tab.set_active(true);
        processes_tab.set_group(Some(&resources_tab));

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabs.add_css_class("module-tabs");
        tabs.append(&resources_tab);
        tabs.append(&processes_tab);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            // Sem isso o Stack força as duas abas a terem a mesma altura
            // (o padrão é homogêneo) — como Processos costuma ter dezenas
            // de linhas, isso inflava Recursos junto, mesmo escondida.
            .vhomogeneous(false)
            .build();
        stack.add_named(&resources_tab_content, Some("resources"));
        stack.add_named(&processes_tab_content, Some("processes"));
        stack.set_visible_child_name("resources");

        let resources_stack = stack.clone();
        resources_tab.connect_clicked(move |button| {
            if button.is_active() {
                resources_stack.set_visible_child_name("resources");
            }
        });
        let processes_stack = stack.clone();
        processes_tab.connect_clicked(move |button| {
            if button.is_active() {
                processes_stack.set_visible_child_name("processes");
            }
        });

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("content-page");
        content.append(
            &gtk::Label::builder()
                .label(gettext("Monitor do Sistema"))
                .xalign(0.0)
                .css_classes(["title-1"])
                .build(),
        );
        content.append(
            &gtk::Label::builder()
                .label(gettext("Uso de recursos e processos em execução"))
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

        Self {
            root,
            status,
            processes_status,
            cpu,
            gpu,
            memory,
            swap,
            disk,
            network,
            list,
            cpu_graph,
            gpu_graph,
            memory_graph,
            swap_graph,
            disk_graph,
            network_graph,
            cpu_cores_flow,
            core_graphs: Rc::new(RefCell::new(Vec::new())),
            gpu_devices_flow,
            gpu_device_graphs: Rc::new(RefCell::new(Vec::new())),
            kill_handlers: KillHandlers::default(),
        }
    }

    pub fn show_metrics(&self, metrics: &SystemMetrics, rates: Option<Rates>) {
        self.cpu.set_label(
            &gettext("{percent}%").replace("{percent}", &format!("{:.1}", metrics.cpu_percent)),
        );
        self.cpu_graph.push(metrics.cpu_percent);
        self.update_core_graphs(&metrics.cpu_per_core);

        if metrics.gpu_percent >= 0.0 {
            self.gpu.set_label(
                &gettext("{percent}%").replace("{percent}", &format!("{:.1}", metrics.gpu_percent)),
            );
            self.gpu_graph.push(metrics.gpu_percent);
        } else {
            self.gpu.set_label(&gettext("Uso indisponível"));
        }
        self.update_gpu_graphs(&metrics.gpu_per_device);

        self.memory.set_label(
            &gettext("{used} de {total}")
                .replace("{used}", &format_bytes(metrics.mem_used))
                .replace("{total}", &format_bytes(metrics.mem_total)),
        );
        self.memory_graph
            .push(percent(metrics.mem_used, metrics.mem_total));

        self.swap.set_label(&if metrics.swap_total == 0 {
            gettext("Sem swap configurado")
        } else {
            gettext("{used} de {total}")
                .replace("{used}", &format_bytes(metrics.swap_used))
                .replace("{total}", &format_bytes(metrics.swap_total))
        });
        self.swap_graph
            .push(percent(metrics.swap_used, metrics.swap_total));

        match rates {
            Some(rates) => {
                self.disk.set_label(
                    &gettext("{read}/s leitura • {write}/s escrita")
                        .replace("{read}", &format_bytes(rates.disk_read_per_sec))
                        .replace("{write}", &format_bytes(rates.disk_write_per_sec)),
                );
                self.disk_graph
                    .push((rates.disk_read_per_sec + rates.disk_write_per_sec) as f64);
                self.network.set_label(
                    &gettext("{rx}/s recebido • {tx}/s enviado")
                        .replace("{rx}", &format_bytes(rates.net_rx_per_sec))
                        .replace("{tx}", &format_bytes(rates.net_tx_per_sec)),
                );
                self.network_graph
                    .push((rates.net_rx_per_sec + rates.net_tx_per_sec) as f64);
            }
            None => {
                self.disk.set_label(&gettext("Calculando…"));
                self.network.set_label(&gettext("Calculando…"));
            }
        }
    }

    /// Os mini-gráficos por núcleo só existem depois da primeira amostra
    /// (é quando dá pra saber quantos núcleos tem); nas chamadas seguintes
    /// só empurra os valores novos nos gráficos já criados.
    fn update_core_graphs(&self, cores: &[f64]) {
        let mut graphs = self.core_graphs.borrow_mut();
        if graphs.is_empty() && !cores.is_empty() {
            for _ in 0..cores.len() {
                // O texto real ("Núcleo N: XX%") é preenchido pelo loop de
                // atualização logo abaixo, na mesma chamada — nunca chega a
                // aparecer vazio na tela.
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .css_classes(["dim-label", "card-title"])
                    .build();
                let graph = Sparkline::new(HISTORY_CAPACITY, Some(100.0));
                graph.widget.set_size_request(56, 24);

                let cell = gtk::Box::new(gtk::Orientation::Vertical, 2);
                cell.add_css_class("card");
                cell.append(&label);
                cell.append(&graph.widget);
                self.cpu_cores_flow.insert(&cell, -1);

                graphs.push(MiniGraph { label, graph });
            }
        }
        for (index, (core, value)) in graphs.iter().zip(cores.iter()).enumerate() {
            core.label.set_label(
                &gettext("Núcleo {n}: {percent}%")
                    .replace("{n}", &index.to_string())
                    .replace("{percent}", &format!("{value:.0}")),
            );
            core.graph.push(*value);
        }
    }

    /// Espelha a grade de núcleos da CPU: cada GPU física recebe seu próprio
    /// percentual e histórico, além do resumo agregado no topo do card.
    fn update_gpu_graphs(&self, devices: &[f64]) {
        let mut graphs = self.gpu_device_graphs.borrow_mut();
        if graphs.len() != devices.len() {
            while let Some(child) = self.gpu_devices_flow.first_child() {
                self.gpu_devices_flow.remove(&child);
            }
            graphs.clear();
            for _ in devices {
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .css_classes(["dim-label", "card-title"])
                    .build();
                let graph = Sparkline::new(HISTORY_CAPACITY, Some(100.0));
                graph.widget.set_size_request(56, 24);

                let cell = gtk::Box::new(gtk::Orientation::Vertical, 2);
                cell.add_css_class("card");
                cell.append(&label);
                cell.append(&graph.widget);
                self.gpu_devices_flow.insert(&cell, -1);

                graphs.push(MiniGraph { label, graph });
            }
        }
        for (index, (device, value)) in graphs.iter().zip(devices.iter()).enumerate() {
            if *value >= 0.0 {
                device.label.set_label(
                    &gettext("GPU {n}: {percent}%")
                        .replace("{n}", &index.to_string())
                        .replace("{percent}", &format!("{value:.0}")),
                );
                device.graph.push(*value);
            } else {
                device.label.set_label(
                    &gettext("GPU {n}: indisponível").replace("{n}", &index.to_string()),
                );
            }
        }
    }

    pub fn show_processes(&self, processes: Vec<ProcessInfo>) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        if processes.is_empty() {
            self.list.append(
                &adw::ActionRow::builder()
                    .title(gettext("Nenhum processo listado"))
                    .build(),
            );
        } else {
            for (index, depth) in build_process_tree(&processes) {
                let process = &processes[index];
                let row = adw::ActionRow::builder()
                    .title(gtk::glib::markup_escape_text(&process.name))
                    .subtitle(gtk::glib::markup_escape_text(
                        &gettext("PID {pid} • {user} • {state}")
                            .replace("{pid}", &process.pid.to_string())
                            .replace("{user}", &process.user)
                            .replace("{state}", &process.state),
                    ))
                    .build();
                // Indenta filhos sob o processo pai, como uma árvore —
                // 12px de margem base mais 20px por nível de profundidade.
                row.set_margin_start(12 + depth as i32 * 20);
                row.add_suffix(&gtk::Label::new(Some(
                    &gettext("{cpu}% • {mem}")
                        .replace("{cpu}", &format!("{:.1}", process.cpu_percent.get()))
                        .replace("{mem}", &format_bytes(process.memory)),
                )));
                let kill = gtk::Button::builder()
                    .icon_name("process-stop-symbolic")
                    .tooltip_text(gettext("Encerrar processo"))
                    .valign(gtk::Align::Center)
                    .css_classes(["flat", "circular", "destructive-action"])
                    .build();
                let handlers = self.kill_handlers.clone();
                let target = process.clone();
                kill.connect_clicked(move |_| {
                    for handler in handlers.borrow().iter() {
                        handler(target.clone());
                    }
                });
                row.add_suffix(&kill);
                self.list.append(&row);
            }
        }
        self.processes_status.set_label(
            &gettext("{count} processo(s)").replace("{count}", &processes.len().to_string()),
        );
    }

    pub fn connect_kill(&self, callback: impl Fn(ProcessInfo) + 'static) {
        self.kill_handlers.borrow_mut().push(Rc::new(callback));
    }
}

impl Default for MonitorPage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rates {
    pub disk_read_per_sec: u64,
    pub disk_write_per_sec: u64,
    pub net_rx_per_sec: u64,
    pub net_tx_per_sec: u64,
}

/// Ordena os processos em árvore (pai antes dos filhos, filhos indentados)
/// e devolve, em ordem de exibição, o índice de cada processo em `processes`
/// junto com sua profundidade. Um processo sem pai visível na lista atual
/// (pai fora do top-250, ou PID 0/1) vira raiz.
fn build_process_tree(processes: &[ProcessInfo]) -> Vec<(usize, usize)> {
    let present: std::collections::HashSet<u32> = processes.iter().map(|p| p.pid).collect();
    let mut children: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::new();
    for (index, process) in processes.iter().enumerate() {
        if process.ppid != process.pid && present.contains(&process.ppid) {
            children.entry(process.ppid).or_default().push(index);
        }
    }
    let child_indices: std::collections::HashSet<usize> =
        children.values().flatten().copied().collect();

    let mut order = Vec::with_capacity(processes.len());
    let mut visited = vec![false; processes.len()];
    let mut stack: Vec<(usize, usize)> = (0..processes.len())
        .filter(|index| !child_indices.contains(index))
        .map(|index| (index, 0))
        .rev()
        .collect();
    while let Some((index, depth)) = stack.pop() {
        if visited[index] {
            continue;
        }
        visited[index] = true;
        order.push((index, depth));
        if let Some(kids) = children.get(&processes[index].pid) {
            for &kid in kids.iter().rev() {
                stack.push((kid, depth + 1));
            }
        }
    }
    // Filhos de um ciclo (não deveria existir em processos reais, mas
    // syscall.Kill entre a leitura e o uso poderia deixar algo estranho)
    // ainda aparecem, só sem indentação.
    for (index, was_visited) in visited.iter().enumerate() {
        if !was_visited {
            order.push((index, 0));
        }
    }
    order
}

fn percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        used as f64 / total as f64 * 100.0
    }
}

fn new_card(caption: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("card");
    card.append(
        &gtk::Label::builder()
            .label(caption)
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build(),
    );
    card
}

fn tab_button(label: &str) -> gtk::ToggleButton {
    gtk::ToggleButton::builder()
        .label(label)
        .css_classes(["flat", "module-tab"])
        .build()
}

fn value_label() -> gtk::Label {
    gtk::Label::builder()
        .label(gettext("Carregando…"))
        .xalign(0.0)
        .build()
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_byte_counts_as_readable_units() {
        assert_eq!(format_bytes(900), "900 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024 * 3), "3.0 MiB");
    }

    fn process(pid: u32, ppid: u32) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid,
            name: format!("proc{pid}"),
            user: "root".into(),
            cpu_percent: lyra_vega_dbus::NotNan::from(0.0),
            memory: 0,
            state: "S".into(),
        }
    }

    #[test]
    fn tree_puts_children_right_after_their_parent_indented() {
        // 1 (root) -> 3, 1 -> 2 -> 4. Input order is deliberately not
        // parent-then-child to prove the tree reorders it; siblings (3
        // then 2, both children of 1) keep their original relative order.
        let processes = vec![process(4, 2), process(1, 0), process(3, 1), process(2, 1)];
        let order = build_process_tree(&processes);
        let pids: Vec<u32> = order.iter().map(|&(i, _)| processes[i].pid).collect();
        let depths: Vec<usize> = order.iter().map(|&(_, d)| d).collect();
        assert_eq!(pids, vec![1, 3, 2, 4]);
        assert_eq!(depths, vec![0, 1, 1, 2]);
    }

    #[test]
    fn process_whose_parent_is_not_in_the_list_becomes_a_root() {
        // ppid=999 doesn't exist in this (truncated top-250) list.
        let processes = vec![process(10, 999)];
        let order = build_process_tree(&processes);
        assert_eq!(order, vec![(0, 0)]);
    }
}
