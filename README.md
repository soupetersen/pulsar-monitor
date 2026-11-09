# Pulsar

A lightweight desktop widget that shows CPU, GPU, RAM, network and disk usage on Windows, Linux and macOS. Written in Rust, with a native UI drawn on the GPU by [gpui](https://www.gpui.rs/) (no WebView).

<p align="center">
  <img src="docs/screenshots/compact-dark.png" alt="Pulsar compact view, dark theme" width="420">
  <img src="docs/screenshots/compact-light.png" alt="Pulsar compact view, light theme" width="420">
</p>

<p align="center">
  <img src="docs/screenshots/expanded-dark.png" alt="Pulsar expanded view, dark theme" width="400">
  <img src="docs/screenshots/expanded-light.png" alt="Pulsar expanded view, light theme" width="400">
</p>

- **Compact mode:** rings for CPU (with temperature or power), GPU (with temperature), RAM and VRAM.
- **Expanded mode:** history charts, per-thread usage, CPU temperature, power and clock, GPU details (temperature, power, VRAM, fan), network, disk, and the top processes by CPU, GPU or RAM, one click away.
- **Themes:** dark and light (sun/moon button), on top of the system backdrop (acrylic on Windows 11, vibrancy on macOS).
- **Tray / menu bar icon:** click to show or hide the widget. The menu has "Always on top", "Start at login" and "Quit" (plus "CPU temperature" on Windows).
- **Languages:** English, or Portuguese when the system is set to Portuguese.

## Platform support

| | Windows 10/11 | Linux | macOS |
|---|---|---|---|
| CPU, RAM, processes, network | ✅ | ✅ | ✅ |
| CPU clock, disk activity | ✅ | ✅ | — |
| CPU power | ✅ | ✅ one-time setup (see below) | — |
| CPU temperature | ✅ AMD and Intel, one-time setup (see below) | ✅ AMD and Intel, no setup | ✅ |
| NVIDIA GPU (usage, temperature, power, VRAM, fan) | ✅ | ✅ | — |
| AMD GPU | ✅ temperature, power %, fan, VRAM | ✅ usage, temperature, power, fan, VRAM | ✅ usage (Intel Macs) |
| Intel GPU | ✅ temperature, power %, VRAM | — | ✅ usage |
| Apple GPU | — | — | ✅ usage, memory in use |
| Top processes by GPU | ✅ | — | — |

Windows is the main target and is tested on real hardware. Linux and macOS builds are compiled on every push, but haven't been tested on real machines yet; reports are welcome.

## Install

Download the file for your system from the [Releases](../../releases) page, or the latest build from the [Actions](../../actions) tab:

- **Windows:** `Pulsar_x.y.z_x64-setup.exe`. Installs for the current user, so no admin rights are needed.
- **Linux:** `.deb` (Debian, Ubuntu), `.rpm` (Fedora, openSUSE) or `.AppImage` (any distribution). The `.deb` and `.rpm` open in the system's software installer with a double click. Pulsar draws with Vulkan, so it needs a Vulkan driver (Mesa's, or the NVIDIA driver), which desktop distributions install by default. On GNOME without the AppIndicator extension (Fedora's default), the tray icon doesn't show; the widget still works, and opening Pulsar again brings it back after it's hidden.
- **macOS:** `Pulsar_x.y.z_aarch64.dmg` (Apple Silicon) or `Pulsar_x.y.z_x86_64.dmg` (Intel). The app isn't signed yet, so the first time open it with right click › Open.

### CPU temperature on Windows (AMD Ryzen, Intel Core)

Windows doesn't expose the CPU temperature to regular programs. Pulsar reads it from the processor through the free, signed [PawnIO](https://pawnio.eu/) driver, which needs administrator rights.

When the temperature is off, a thermometer icon shows up in the widget. Click it and accept the administrator prompt once. Pulsar then registers a small helper that starts at logon and reads only the temperature register, so the widget itself keeps running without admin rights. If PawnIO isn't installed, the same icon opens its website.

The helper runs from a copy under `C:\Program Files\Pulsar`, which only administrators can modify. You can turn it off again from the tray menu.

On Linux and macOS the temperature comes from the system sensors and needs no setup.

### CPU power on Linux

Since 2020 the kernel lets only root read the CPU energy counter (RAPL), because it could be used as a side channel (Platypus). When that's the case, a lightning icon shows up in place of the CPU power. Click it and enter your password once: Pulsar adds a udev rule (`/etc/udev/rules.d/70-pulsar-rapl.rules`) that makes the counter readable, now and after every boot. To undo it, delete that file and reboot.

## Build

```sh
cargo run                 # development
cargo build --release     # target/release/pulsar(.exe)
packaging/package.sh target/release/pulsar dist   # installers for this system, in dist/
```

On Linux, install the build dependencies first (Debian/Ubuntu):

```sh
sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev \
  libx11-dev libx11-xcb-dev libxcb1-dev libxcb-xkb-dev libfontconfig-dev libfreetype-dev
```

Packaging needs `makensis` (NSIS) on Windows, and `dpkg-deb` and `rpmbuild` on Linux.

## Releasing

```sh
scripts/release.sh patch                        # 0.3.0 -> 0.3.1, all platforms
scripts/release.sh minor --platforms windows    # only the Windows installer
scripts/release.sh 1.0.0 --platforms windows,linux
scripts/release.sh patch --dry-run              # show what would happen
```

Pulsar updates itself: it checks the latest published release shortly after starting, every hour and when the widget is shown again, and shows an update icon when a newer version exists. Clicking it downloads, installs and restarts. Updates are signed with minisign; the release workflow signs them with the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets (the key of the Tauri versions, so those update to this one), and the app only accepts packages signed with that key (its public half is in `src/shell/updater.rs`). Losing the private key means installed copies can no longer update, so keep a backup of it.

The script bumps the version in `Cargo.toml` and `Cargo.lock`, commits `Release x.y.z`, tags `vx.y.z`, pushes both, and starts the Release workflow. The workflow builds the installers of the chosen platforms and attaches them to a draft release, which is published after a quick look (`gh release edit vx.y.z --draft=false`). Regular commits only run the Build workflow.

## Project layout

```
src/
├── main.rs                    entry point (and the CPU temperature helper's)
├── ui/                        gpui views: compact and expanded, theme, strings
├── shell/                     window, tray, single instance, start at login,
│                              settings, updater
└── core/                      data collection, independent of the UI
    ├── sampler.rs             collects everything once per second
    ├── platform/              per-OS data: windows.rs, linux.rs, macos.rs
    └── cpu_sensor.rs          CPU temperature helper (Windows)
packaging/                     installers: NSIS, .app/dmg, deb, rpm, AppImage
```

## Data sources

| Data | Source |
|---|---|
| CPU, RAM, processes, network (all) | `sysinfo` crate |
| NVIDIA GPU (Windows, Linux) | NVML via `nvml-wrapper` |
| AMD and Intel GPUs (Windows) | D3DKMT adapter performance data (the same source Task Manager uses) |
| Clock, disk, per-process GPU usage (Windows) | PDH performance counters |
| CPU power (Windows) | "Energy Meter" counters (RAPL), no driver needed |
| CPU temperature (Windows) | PawnIO driver with its signed `AMDFamily17` module (Tctl register) or `IntelMSR` module (package thermal status) |
| CPU temperature, clock, power, disk (Linux) | hwmon (`k10temp`, `zenpower`, `coretemp`), cpufreq, RAPL powercap, `/proc/diskstats` |
| AMD GPU (Linux) | `amdgpu` sysfs files |
| CPU temperature (macOS) | System sensors through `sysinfo` |
| GPU (macOS) | IOKit `IOAccelerator` performance statistics |

The sampler (`src/core/sampler.rs`) runs once per second and only collects what is on screen:
- **Compact mode:** totals only.
- **Process list open:** that list's processes too.
- **Hidden in the tray:** nothing at all.

## Performance notes

- Value changes animate in 8 discrete steps over 0.4 s: it reads as motion while redrawing only 8 times per update.
- The window is redrawn only when something changes (a new sample, a hover, an animation step), and resized only when its content's size or the screen scale changes.

Measured on Windows 11 with a Ryzen 7 9800X3D and an RTX 4080 SUPER on a 4K display:

| State | CPU (of all threads) | Memory (private working set) |
|---|---|---|
| Compact | ~0.07–0.1% | ~50 MB |

About 20 MB of it is NVIDIA's NVML library, loaded only when an NVIDIA GPU is present. On Fedora the app uses about 47 MB. The Tauri versions (up to 0.4) used about 300 MB on Windows, most of it in WebView2, and 100 MB on Linux.

## Third-party

`assets/pawnio/AMDFamily17.bin` and `IntelMSR.bin` are signed modules from [PawnIO.Modules](https://github.com/namazso/PawnIO.Modules), licensed under LGPL-2.1 (see `COPYING-LGPL-2.1` next to it).
