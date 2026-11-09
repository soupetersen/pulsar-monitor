# Pulsar

A lightweight desktop widget for Windows 11 that shows CPU, GPU, RAM, network and disk usage. Built with Tauri 2 (Rust) and Svelte.

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
- **Themes:** dark and light (sun/moon button), on top of the Windows 11 Mica backdrop.
- **Tray icon:** click to show or hide the widget. The menu has "Always on top", "Start with Windows", "CPU temperature" and "Quit".
- **Languages:** English, or Portuguese when Windows is set to Portuguese.

## Install

Download `Pulsar_x.y.z_x64-setup.exe` from the [Releases](../../releases) page and run it. It installs for the current user, so no admin rights are needed. It requires the WebView2 runtime, which ships with Windows 11.

### CPU temperature (AMD Ryzen, Intel Core)

Windows doesn't expose the CPU temperature to regular programs. Pulsar reads it from the processor through the free, signed [PawnIO](https://pawnio.eu/) driver, which needs administrator rights.

When the temperature is off, a thermometer icon shows up in the widget. Click it and accept the administrator prompt once. Pulsar then registers a small helper that starts at logon and reads only the temperature register, so the widget itself keeps running without admin rights. If PawnIO isn't installed, the same icon opens its website.

The helper runs from a copy under `C:\Program Files\Pulsar`, which only administrators can modify. You can turn it off again from the tray menu.

## Build

```sh
npm install
npm run tauri dev                       # development
npm run tauri build -- --no-bundle      # src/core/target/release/pulsar.exe
npm run tauri build                     # NSIS installer in src/core/target/release/bundle/nsis
```

## Project layout

```
src/
├── ui/      Svelte interface: routes, components, styles
└── core/    Rust (Tauri): data collection, window, tray, CPU sensor helper
```

## Data sources

| Data | Source |
|---|---|
| CPU, RAM, processes, network | `sysinfo` crate |
| NVIDIA GPU (usage, temperature, power, VRAM, fan, clock) | NVML via `nvml-wrapper` (uses the driver's `nvml.dll`) |
| AMD and Intel GPUs (temperature, power %, fan RPM, VRAM) | Windows D3DKMT adapter performance data (the same source Task Manager uses) |
| Effective CPU clock, disk activity, per-process GPU usage | Windows PDH performance counters (the same source Task Manager uses) |
| CPU package power | Windows "Energy Meter" counters (RAPL), no driver needed |
| CPU temperature (AMD, Intel) | PawnIO driver with its signed `AMDFamily17` module (Tctl register) or `IntelMSR` module (package thermal status) |

The sampler (`src/core/src/sampler.rs`) runs once per second and only collects what is on screen:
- **Compact mode:** totals only.
- **Process list open:** that list's processes too.
- **Hidden in the tray:** nothing at all, and the WebView switches to its low-memory mode.

## Performance notes

- Value transitions use `steps(8)` instead of smooth easing. With continuous transitions the WebView repainted at 60–144 fps after every update, which cost about 25% of a core on a 4K display. With steps it costs about 3%.
- The window is sized in physical pixels from the WebView's own `devicePixelRatio`, which can differ from the window scale factor on mixed-DPI multi-monitor setups.

Measured on a Ryzen 7 9800X3D with an RTX 4080 SUPER on a 4K display:

| State | CPU (of all threads) | Private memory |
|---|---|---|
| Compact | ~0.2% | ~200 MB |
| Expanded with the GPU process list open | ~0.4–0.6% | ~240 MB |
| Hidden in the tray | ~0% | ~220 MB |

Most of the memory belongs to WebView2. The Rust process itself uses about 35 MB.

## Third-party

`src/core/resources/pawnio/AMDFamily17.bin` and `IntelMSR.bin` are signed modules from [PawnIO.Modules](https://github.com/namazso/PawnIO.Modules), licensed under LGPL-2.1 (see `COPYING-LGPL-2.1` next to it).
