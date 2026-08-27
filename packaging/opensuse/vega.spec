# Empacotamento Linux do Vega XFCE. Versionamento via --define version
# (ver Version abaixo); ver packaging/obs/vega-xfce.spec para a variante
# consumida pelo OBS via tar_scm.
%{!?version: %define version 0.0.0}
Name:           vega-xfce
Version:        %{version}
Release:        1%{?dist}
Summary:        Centro de controle para Linux
License:        GPL-3.0-only
URL:            https://github.com/lyra-os-linux/vega-xfce
Source0:        vega-src.tar.gz

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
%setup -q -c -n vega-src

%build
cd vega-xfce
VEGA_VERSION=%{version} cargo build --release --locked

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
