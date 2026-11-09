#!/usr/bin/env bash
# Builds the installers for this system from a release binary:
#
#   packaging/package.sh <binary> <out dir>
#
#   Windows  Pulsar_<v>_x64-setup.exe                  (NSIS: makensis)
#   macOS    Pulsar_<v>_<arch>.dmg, Pulsar_<arch>.app.tar.gz (the update package)
#   Linux    Pulsar_<v>_amd64.deb, Pulsar-<v>-1.x86_64.rpm, Pulsar_<v>_amd64.AppImage
#
# The names, package names and install paths are the ones of the Tauri
# versions, so these packages update those installs in place.
set -euo pipefail

binary=$(realpath "$1")
out=$(mkdir -p "$2" && realpath "$2")
root=$(cd "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
icons="$root/assets/icons"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

windows() {
  makensis -V2 -DVERSION="$version" -DBINARY="$(cygpath -w "$binary")" -DICON="$(cygpath -w "$icons/icon.ico")" \
    -DOUTFILE="$(cygpath -w "$out/Pulsar_${version}_x64-setup.exe")" "$(cygpath -w "$root/packaging/windows/installer.nsi")"
}

macos() {
  local arch app
  # Named like the Tauri builds: aarch64 or x86_64.
  arch=$(lipo -archs "$binary" | sed 's/arm64/aarch64/')
  app="$work/Pulsar.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "$binary" "$app/Contents/MacOS/pulsar"
  cp "$icons/icon.icns" "$app/Contents/Resources/icon.icns"
  sed "s/@VERSION@/$version/g" "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
  # Ad-hoc signature: Apple Silicon only runs signed code.
  codesign --force --deep --sign - "$app"

  # The update package (what the updater swaps in) and the disk image.
  tar -czf "$out/Pulsar_${arch}.app.tar.gz" -C "$work" Pulsar.app
  mkdir "$work/dmg"
  cp -R "$app" "$work/dmg/"
  ln -s /Applications "$work/dmg/Applications"
  hdiutil create -quiet -volname Pulsar -srcfolder "$work/dmg" -ov -format UDZO "$out/Pulsar_${version}_${arch}.dmg"
}

# Shared libraries the binary links to, as Debian packages (on the build
# machine): a small dpkg-shlibdeps.
deb_depends() {
  local libs pkgs
  libs=$(objdump -p "$binary" | awk '/NEEDED/ {print $2}')
  pkgs=$(for lib in $libs; do
    dpkg -S "*/$lib" 2>/dev/null | head -1 | cut -d: -f1
  done | sort -u | grep -v '^libc6$' || true)
  # Loaded at runtime (dlopen) by gpui: Vulkan for rendering, xkbcommon
  # for keyboards.
  printf '%s\n' $pkgs libc6 libvulkan1 libxkbcommon0 libxkbcommon-x11-0 | sort -u | paste -sd, | sed 's/,/, /g'
}

# The files of a Linux install, under $1.
linux_tree() {
  install -Dm755 "$binary" "$1/usr/bin/pulsar"
  install -Dm644 "$root/packaging/linux/Pulsar.desktop" "$1/usr/share/applications/Pulsar.desktop"
  install -Dm644 "$icons/32x32.png" "$1/usr/share/icons/hicolor/32x32/apps/pulsar.png"
  install -Dm644 "$icons/128x128.png" "$1/usr/share/icons/hicolor/128x128/apps/pulsar.png"
  install -Dm644 "$icons/128x128@2x.png" "$1/usr/share/icons/hicolor/256x256@2/apps/pulsar.png"
}

linux() {
  local deb="$work/deb" size
  linux_tree "$deb"
  size=$(du -sk "$deb/usr" | cut -f1)
  mkdir -p "$deb/DEBIAN"
  cat > "$deb/DEBIAN/control" <<EOF
Package: pulsar
Version: $version
Architecture: amd64
Installed-Size: $size
Maintainer: Thiago Petersen
Priority: optional
Section: utils
Depends: $(deb_depends)
Recommends: mesa-vulkan-drivers
Description: Lightweight system monitor desktop widget (CPU, GPU, RAM)
EOF
  cat "$deb/DEBIAN/control"
  dpkg-deb --root-owner-group --build "$deb" "$out/Pulsar_${version}_amd64.deb"

  # rpm: rpmbuild finds the libraries it needs (as sonames, which Fedora
  # and openSUSE provide) by itself.
  local rpm="$work/rpm"
  mkdir -p "$rpm/root"
  linux_tree "$rpm/root"
  sed "s/@VERSION@/$version/g" "$root/packaging/linux/pulsar.spec" > "$rpm/pulsar.spec"
  rpmbuild -bb --quiet --define "_topdir $rpm" --define "_rpmdir $rpm/out" --buildroot "$rpm/buildroot" \
    --define "tree $rpm/root" "$rpm/pulsar.spec"
  cp "$rpm"/out/x86_64/pulsar-"$version"-1.x86_64.rpm "$out/Pulsar-${version}-1.x86_64.rpm"

  # AppImage: the binary and its desktop entry; the libraries come from
  # the system, like for the deb and rpm.
  local appdir="$work/Pulsar.AppDir"
  linux_tree "$appdir"
  ln -s usr/bin/pulsar "$appdir/AppRun"
  cp "$root/packaging/linux/Pulsar.desktop" "$appdir/pulsar.desktop"
  cp "$icons/128x128.png" "$appdir/pulsar.png"
  curl -fsSL -o "$work/appimagetool" \
    https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
  chmod +x "$work/appimagetool"
  ARCH=x86_64 "$work/appimagetool" --appimage-extract-and-run --no-appstream "$appdir" \
    "$out/Pulsar_${version}_amd64.AppImage"
}

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) windows ;;
  Darwin) macos ;;
  Linux) linux ;;
  *) echo "package.sh: unknown system $(uname -s)" >&2; exit 1 ;;
esac
ls -l "$out"
