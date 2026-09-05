# Vega — Control Center

> [!WARNING]
> Este é o fork experimental do Vega para o flavor XFCE. Ele compartilha o
> daemon e o contrato D-Bus do Vega e mantém GTK4/libadwaita como base visual
> durante a migração.

*[Leia em português](README.pt-br.md)*

Vega XFCE is a native control center built exclusively for openSUSE. It brings
software, hardware, kernel, network, backup, user, and service administration
into a single interface integrated with XFCE. It complements XFCE Settings
with administration tasks that would otherwise require separate tools such as
`zypper`, `nmcli`, `systemctl`, and configuration-file editors.

The project provides a graphical interface built with Rust and
GTK4/libadwaita, plus a terminal interface built with Bash and `dialog`. Both
use the same privileged daemon and D-Bus contract.

Licensed under GPL-3.0. This repository hosts `vega-xfce` and the
product-wide docs/scripts; the other components each have their own
repository — see [Architecture](#architecture) below.

## Features

- dashboard with system-health information and shortcuts;
- Zypper packages, Flatpak applications, updates, and repositories;
- optional snapshots with Snapper or Timeshift, and backups with Restic;
- hardware inventory, kernel, and bootloader;
- storage, date, time, and locale;
- network, Wi-Fi, Bluetooth, firewall, VPN, proxy, and IPv4;
- users, services, logs, and live process monitoring;
- wallpaper, screen-lock preferences, and a multi-provider AI assistant.

Driver installation and switching, including NVIDIA and optional hardware firmware,
are no longer offered by Vega. Hardware inventory and GPU monitoring remain available.

Features backed by optional programs are shown as unavailable when their
dependency is missing without preventing the other pages from working.

## Architecture

Vega is split across several repositories under
[lyra-os-linux](https://github.com/lyra-os-linux). This repository
(`vega-xfce`) hosts `vega-xfce` plus the docs/scripts that span the whole
product; each other component has its own repository and release cycle:

| Component | Technology | Role | Repository |
| --- | --- | --- | --- |
| `vega-xfce` | Rust, GTK4, and libadwaita | Unprivileged XFCE graphical interface | this repo |
| `vega-cli` | Bash and `dialog` | Terminal interface for local or SSH use | [lyra-os-linux/vega-cli](https://github.com/lyra-os-linux/vega-cli) |
| `vega-web` | Rust, axum | HTTPS panel for LAN-only administration | [lyra-os-linux/vega-web](https://github.com/lyra-os-linux/vega-web) |
| `vegad` | Go | Daemon that performs authorized system operations | [lyra-os-linux/vegad](https://github.com/lyra-os-linux/vegad) |
| `lyra-vega-dbus` + `dbus/` | Rust/zbus + introspection XML | Typed D-Bus client and the public `org.lyraos.Vega1.*` contract, shared by the Rust frontends | [lyra-os-linux/lyra-vega-dbus](https://github.com/lyra-os-linux/lyra-vega-dbus) |

Building the full product locally means cloning the repos above as
siblings (e.g. under the same parent directory) — see
[CONTRIBUTING.md](CONTRIBUTING.md).

`vegad` uses the system bus and is activated on demand by D-Bus. It releases
the bus name and exits after two minutes without activity. Read-only queries do
not require authentication; system-changing actions are protected by granular
polkit rules. The graphical interface never needs to run as root.

Vega CLI is aimed primarily at headless servers. Its `vega` entrypoint requires
an interactive terminal and runs as the session user; polkit requests
authentication only when a privileged action is performed.

## Installing on openSUSE

Vega supports openSUSE only. On openSUSE Leap 16.0, the recommended installation
method uses the
[`home:rodrigosbrito:vega`](https://build.opensuse.org/project/show/home:rodrigosbrito:vega)
repository on the openSUSE Build Service:

### Add the OBS repository and install with Zypper

Add the Vega repository:

```sh
sudo zypper addrepo --refresh \
  https://download.opensuse.org/repositories/home:/rodrigosbrito:/vega/openSUSE_Leap_16.0/ \
  vega-obs
```

Refresh its metadata and import the OBS signing key:

```sh
sudo zypper --gpg-auto-import-keys refresh vega-obs
```

Install the graphical interface, daemon, and terminal interface:

```sh
sudo zypper install vega-xfce vegad
```

`vegad` is activated automatically over D-Bus when an interface needs it; it
does not need to be started manually.

To update Vega later:

```sh
sudo zypper refresh vega-obs
sudo zypper update
```

### Headless installation

To install only the daemon and terminal interface on a headless machine:

```sh
curl -fsSL https://raw.githubusercontent.com/lyra-os-linux/vega/main/scripts/install-obs.sh \
  | sudo env VEGA_CLI_ONLY=1 bash
```

Or, if the repository is already configured:

```sh
sudo zypper install vegad vega-cli
```

After installation, open the graphical interface from the application menu or
run `vega-xfce`.

### Release RPMs

Alternatively, `scripts/install.sh` downloads RPMs from the latest GitHub
release of each component's repository (`vega`, `vegad`, `vega-cli`)
without configuring the OBS repository. A specific tag can be selected with
`VEGA_VERSION=vX.Y.Z` (used against all three repos, so it only works if
their releases share that tag); these standalone RPMs are still installed
as unsigned packages.

## Uninstalling

```sh
sudo bash scripts/uninstall.sh
```

The script removes any installed Vega packages listed by that installer.
Set `VEGA_PURGE=1` to also delete backup configuration under `/etc/vega` and
exported logs under `/var/log/vega`.

Per-user assistant preferences in
`~/.local/share/vega-gtk/ai-settings.json` are preserved.

## Development

Prerequisites:

- Rust 1.92 or newer, GTK4, and libadwaita;
- openSUSE with systemd, D-Bus, and polkit for integration testing
  (needs `vegad` installed/running — see its own repository).

Validate the Rust interface and client from the repository root:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

The daemon (`vegad`) has its own repository, tests, and validation —
see [lyra-os-linux/vegad](https://github.com/lyra-os-linux/vegad).

Run the graphical interface during development:

```sh
cargo run --manifest-path vega-xfce/Cargo.toml --bin vega-xfce
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines,
[vega-xfce/README.md](vega-xfce/README.md) for interface details, and
[lyra-vega-dbus](https://github.com/lyra-os-linux/lyra-vega-dbus) for the
D-Bus contract.

## Tested openSUSE versions

- openSUSE Leap 16
- openSUSE Tumbleweed

## Known limitations

- Other Linux distributions are not supported.
- Zypper and Flatpak progress is reported per step rather than per byte
  transferred.
- Snapper and Timeshift are optional. Advanced diff and retention features
  remain Snapper-specific.
- Wi-Fi, Bluetooth, screen settings, and the AI assistant belong to a graphical
  session and are not included in Vega CLI.

## Assistant privacy

The assistant is optional. Keys are stored in the session Secret Service, and
system-changing actions are presented as proposals before anything is executed.
See [docs/ai-privacidade.md](docs/ai-privacidade.md) for details.
