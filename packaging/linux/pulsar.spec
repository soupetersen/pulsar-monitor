# Built by packaging/package.sh, which fills in the version and passes the
# install tree as %{tree}.
%global debug_package %{nil}
# Same file list as the Tauri versions (no /usr/lib/.build-id links).
%global _build_id_links none
Name:           pulsar
Version:        @VERSION@
Release:        1
Summary:        Lightweight system monitor desktop widget (CPU, GPU, RAM)
License:        MIT
URL:            https://github.com/soupetersen/pulsar-monitor
# Loaded at runtime (dlopen) by gpui, so not found automatically.
Requires:       vulkan-loader
Requires:       libxkbcommon
Requires:       libxkbcommon-x11

%description
Pulsar shows CPU, GPU, RAM and VRAM usage in a small always-on-top widget.

%install
cp -a %{tree}/. %{buildroot}/

%files
/usr/bin/pulsar
/usr/share/applications/Pulsar.desktop
/usr/share/icons/hicolor/32x32/apps/pulsar.png
/usr/share/icons/hicolor/128x128/apps/pulsar.png
/usr/share/icons/hicolor/256x256@2/apps/pulsar.png
