# Vega — Centro de Controle

*[Read in English](README.md)*

Vega é um centro de controle nativo e exclusivo para openSUSE que reúne
administração de software, hardware, kernel, rede, backups, usuários e serviços
em uma interface integrada ao GNOME. Ele complementa as Configurações do GNOME
com tarefas de administração que normalmente exigem ferramentas separadas,
como `zypper`, `nmcli`, `systemctl` e editores de arquivos de configuração.

O projeto oferece uma interface gráfica em Rust com GTK4/libadwaita e uma
interface de terminal em Bash com `dialog`. As duas usam o mesmo daemon
privilegiado e o mesmo contrato D-Bus.

Licenciado sob GPL-3.0. Este repositório hospeda o `vega-gtk` e a
documentação/scripts que atravessam o produto inteiro; os outros
componentes têm cada um seu próprio repositório — ver
[Arquitetura](#arquitetura) abaixo.

## Recursos

- painel com resumo da saúde do sistema e atalhos;
- pacotes Zypper, aplicativos Flatpak, atualizações e repositórios;
- snapshots opcionais com Snapper ou Timeshift e backups com Restic;
- inventário de hardware, kernel e bootloader;
- armazenamento, data, hora e idioma;
- rede, Wi-Fi, Bluetooth, firewall, VPN, proxy e IPv4;
- usuários, serviços, logs e monitor de processos em tempo real;
- papel de parede, bloqueio de tela e assistente com múltiplos provedores de IA.

Recursos que dependem de programas opcionais aparecem como indisponíveis quando
a dependência não está instalada, sem impedir o uso das outras telas.

A instalação e a troca de drivers, incluindo NVIDIA e firmware opcional, foram
removidas do Vega. O inventário de hardware e a monitoração da GPU continuam disponíveis.


## Arquitetura

O Vega é dividido em vários repositórios sob
[lyra-os-linux](https://github.com/lyra-os-linux). Este repositório
(`vega`) hospeda o `vega-gtk` mais a documentação/scripts que atravessam o
produto inteiro; cada outro componente tem seu próprio repositório e ciclo
de release:

| Componente | Tecnologia | Função | Repositório |
| --- | --- | --- | --- |
| `vega-gtk` | Rust, GTK4 e libadwaita | Interface gráfica executada pelo usuário, sem privilégios | este repo |
| `vega-cli` | Bash e `dialog` | Interface de terminal para uso local ou por SSH | [lyra-os-linux/vega-cli](https://github.com/lyra-os-linux/vega-cli) |
| `vega-web` | Rust, axum | Painel HTTPS para administração restrita à LAN | [lyra-os-linux/vega-web](https://github.com/lyra-os-linux/vega-web) |
| `vegad` | Go | Daemon que executa operações de sistema autorizadas | [lyra-os-linux/vegad](https://github.com/lyra-os-linux/vegad) |
| `lyra-vega-dbus` + `dbus/` | Rust/zbus + XML de introspecção | Cliente D-Bus tipado e o contrato público `org.lyraos.Vega1.*`, compartilhados pelas interfaces Rust | [lyra-os-linux/lyra-vega-dbus](https://github.com/lyra-os-linux/lyra-vega-dbus) |

Para buildar o produto completo localmente é preciso clonar os repositórios
acima como irmãos (ex.: no mesmo diretório pai) — ver
[CONTRIBUTING.md](CONTRIBUTING.md).

O `vegad` usa o barramento de sistema e é ativado sob demanda pelo D-Bus. Após
dois minutos sem atividade, ele libera o nome do barramento e encerra. Consultas
são feitas sem autenticação; ações que alteram o sistema são protegidas por
regras polkit específicas. A interface gráfica nunca precisa ser executada como
root.

O Vega CLI é voltado principalmente a servidores sem ambiente gráfico. Seu
entrypoint `vega` requer um terminal interativo e roda como o usuário da sessão;
o polkit solicita autenticação apenas ao executar uma ação privilegiada.

## Instalação no openSUSE

O Vega oferece suporte apenas ao openSUSE. Para o openSUSE Leap 16.0, a
instalação recomendada usa o repositório
[`home:rodrigosbrito:vega`](https://build.opensuse.org/project/show/home:rodrigosbrito:vega)
no openSUSE Build Service:

### Adicionar o repositório OBS e instalar com Zypper

Adicione o repositório do Vega:

```sh
sudo zypper addrepo --refresh \
  https://download.opensuse.org/repositories/home:/rodrigosbrito:/vega/openSUSE_Leap_16.0/ \
  vega-obs
```

Atualize os metadados e importe a chave de assinatura do OBS:

```sh
sudo zypper --gpg-auto-import-keys refresh vega-obs
```

Instale a interface gráfica, o daemon e a interface de terminal:

```sh
sudo zypper install vega-gtk vegad vega-cli
```

O `vegad` é ativado automaticamente pelo D-Bus quando uma interface precisa
dele; não é necessário iniciá-lo manualmente.

Para atualizar o Vega depois:

```sh
sudo zypper refresh vega-obs
sudo zypper update
```

### Instalação headless

Para instalar apenas o daemon e a interface de terminal em uma máquina
headless:

```sh
curl -fsSL https://raw.githubusercontent.com/lyra-os-linux/vega/main/scripts/install-obs.sh \
  | sudo env VEGA_CLI_ONLY=1 bash
```

Ou, com o repositório já configurado:

```sh
sudo zypper install vegad vega-cli
```

Depois da instalação, abra a interface gráfica pelo menu de aplicativos ou
execute `vega-gtk`. Para a interface de terminal, execute `vega`.

### RPMs de releases

Como alternativa, `scripts/install.sh` baixa os RPMs da release mais recente
de cada repositório de componente (`vega`, `vegad`, `vega-cli`) sem
configurar o repositório OBS. É possível escolher uma tag com
`VEGA_VERSION=vX.Y.Z` (aplicada aos três repositórios, então só funciona se
as releases deles compartilharem essa tag); esses RPMs avulsos ainda são
instalados como pacotes não assinados.

## Desinstalação

```sh
sudo bash scripts/uninstall.sh
```

O script remove os pacotes `vega-gtk`, `vegad` e `vega-cli` que estiverem
instalados. Use `VEGA_PURGE=1` para também apagar configurações de backup em
`/etc/vega` e logs exportados em `/var/log/vega`.

As preferências por usuário do assistente, em
`~/.local/share/vega-gtk/ai-settings.json`, são preservadas.

## Desenvolvimento

Pré-requisitos:

- Rust 1.92 ou mais recente, GTK4 e libadwaita;
- openSUSE com systemd, D-Bus e polkit para testes de integração
  (precisa do `vegad` instalado/rodando — ver o repositório próprio dele).

Valide a interface e o cliente Rust a partir da raiz:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

O daemon (`vegad`) tem seu próprio repositório, testes e validação — ver
[lyra-os-linux/vegad](https://github.com/lyra-os-linux/vegad).

Para executar a interface durante o desenvolvimento:

```sh
cargo run --manifest-path vega-gtk/Cargo.toml
```

Consulte [CONTRIBUTING.md](CONTRIBUTING.md) para as regras de contribuição,
[vega-gtk/README.md](vega-gtk/README.md) para detalhes da interface e
[lyra-vega-dbus](https://github.com/lyra-os-linux/lyra-vega-dbus) para o
contrato D-Bus.

## Versões do openSUSE testadas

- openSUSE Leap 16
- openSUSE Tumbleweed

## Limitações conhecidas

- Outras distribuições Linux não são suportadas.
- O progresso de operações com Zypper e Flatpak é informado por etapa, não por
  bytes transferidos.
- Snapper e Timeshift são opcionais. Recursos avançados de diff e retenção
  continuam específicos do Snapper.
- Wi-Fi, Bluetooth, tela e assistente de IA pertencem à sessão gráfica e não
  fazem parte do Vega CLI.

## Privacidade do assistente

O assistente é opcional. Chaves são armazenadas no Secret Service da sessão, e
ações que modificam o sistema são apresentadas como propostas antes de qualquer
execução. Consulte [docs/ai-privacidade.md](docs/ai-privacidade.md) para detalhes.
