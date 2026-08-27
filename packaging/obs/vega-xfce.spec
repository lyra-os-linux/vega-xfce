# Empacotamento do vega-xfce para o openSUSE Build Service
# (home:rodrigosbrito:vega), separado do pacote vega-gtk.
# Cópia de packaging/opensuse/vega.spec adaptada só no Source0/%setup pra
# bater com o tarball que o _service (tar_scm) deste mesmo diretório
# gera — nome com sufixo de versão e diretório interno próprio, ao invés
# do tar "achatado" usado pelo empacotamento local. Resto do spec é
# idêntico ao de packaging/opensuse/.
#
# Version literal (não %%{version}/%%define) — o serviço set_version deste
# diretório faz substituição textual simples na linha "Version:" e não
# entende macro, então precisa achar um valor literal aqui pra reescrever.
Name:           vega-xfce
Version:        0
Release:        1%{?dist}
Summary:        Centro de controle para Linux
License:        GPL-3.0-only
URL:            https://github.com/lyra-os-linux/vega-xfce
Source0:        vega-src-%{version}.tar
# vendor.tar.gz gerado pelo _service cargo_vendor (rede exigida, que a VM
# de build do OBS não tem — sem isso, "cargo build" trava tentando baixar
# crates de index.crates.io e falha). Traz .cargo/config.toml + Cargo.lock
# + vendor/ prontos pra extrair na raiz do workspace.
Source1:        vendor.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(libadwaita-1)
BuildRequires:  gettext-tools
Requires:       vegad
Requires:       secret-tool
Requires:       xfconf

Recommends:     flatpak
Recommends:     restic

%description
Interface do Vega para XFCE, construída com Rust e GTK4.

%prep
%setup -q -n vega-src-%{version}
# .cargo/config.toml + vendor/ vão na raiz do workspace, junto do
# Cargo.toml — é onde o cargo procura por padrão.
# O vendor.tar.gz pode ter sido gerado numa release anterior. Preserve o
# Cargo.lock da tag atual; o tar fornece apenas a configuração offline e os
# crates vendorizados.
tar --anchored --exclude=Cargo.lock -xzf %{SOURCE1}

%build
cd vega-xfce
VEGA_VERSION=%{version} cargo build --release --locked --offline

%install
install -Dm755 target/release/vega-xfce \
  %{buildroot}%{_bindir}/vega-xfce

install -Dm644 packaging/vega/vega.desktop \
  %{buildroot}%{_datadir}/applications/vega-xfce.desktop
install -Dm644 packaging/vega/vega.svg \
  %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/vega.svg
install -Dm644 packaging/vega/icons/lyra-updates-symbolic.svg \
  %{buildroot}%{_datadir}/icons/hicolor/symbolic/apps/lyra-updates-symbolic.svg
for locale in en_US pt_BR es_ES; do
  install -Dm644 "vega-xfce/po/locale/${locale}/LC_MESSAGES/vega-gtk.mo" \
    "%{buildroot}%{_datadir}/locale/${locale}/LC_MESSAGES/vega-gtk.mo"
done

%files
%{_bindir}/vega-xfce
%{_datadir}/applications/vega-xfce.desktop
%{_datadir}/icons/hicolor/scalable/apps/vega.svg
%{_datadir}/icons/hicolor/symbolic/apps/lyra-updates-symbolic.svg
%lang(en) %{_datadir}/locale/en_US/LC_MESSAGES/vega-gtk*.mo
%lang(pt_BR) %{_datadir}/locale/pt_BR/LC_MESSAGES/vega-gtk*.mo
%lang(es) %{_datadir}/locale/es_ES/LC_MESSAGES/vega-gtk*.mo

%changelog
