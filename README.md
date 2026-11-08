# Widget Monitor

A lightweight desktop widget for Windows 11 that shows CPU, GPU, RAM, network and disk usage. Built with Tauri 2 (Rust) and Svelte.

- **Compact mode:** rings for CPU, GPU (with temperature), RAM and VRAM.
- **Expanded mode:** history charts, per-thread usage, GPU details (temperature, power, VRAM, fan), network, disk, and the top processes by CPU, GPU or RAM, one click away.
- **Themes:** dark and light (sun/moon button), on top of the Windows 11 Mica backdrop.
- **Tray icon:** click to show or hide the widget. The menu has "Always on top", "Start with Windows" and "Quit".
- **Languages:** English, or Portuguese when Windows is set to Portuguese.

## Install

Download `Widget Monitor_x.y.z_x64-setup.exe` from the [Releases](../../releases) page and run it. It installs for the current user, so no admin rights are needed. It requires the WebView2 runtime, which ships with Windows 11.

## Build

```sh
npm install
npm run tauri dev                       # development
npm run tauri build -- --no-bundle      # src-tauri/target/release/widgets.exe
npm run tauri build                     # NSIS installer in src-tauri/target/release/bundle/nsis
```

## Data sources

| Data | Source |
|---|---|
| CPU, RAM, processes, network | `sysinfo` crate |
| NVIDIA GPU (usage, temperature, power, VRAM, fan, clock) | NVML via `nvml-wrapper` (uses the driver's `nvml.dll`) |
| Effective CPU clock, disk activity, per-process GPU usage | Windows PDH performance counters (the same source Task Manager uses) |

The sampler (`src-tauri/src/sampler.rs`) runs once per second and only collects what is on screen:
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
