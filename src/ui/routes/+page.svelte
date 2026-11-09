<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { currentMonitor, getCurrentWindow, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/window";
  import Compact from "$lib/Compact.svelte";
  import Expanded from "$lib/Expanded.svelte";
  import type { CpuTempStatus, Panel, Stats, Theme } from "$lib/types";

  const HISTORY = 30;
  const win = getCurrentWindow();

  let stats = $state<Stats | null>(null);
  let expanded = $state(false);
  let panel = $state<Panel>("none");
  let theme = $state<Theme>(loadTheme());
  let onTop = $state(true);
  let cpuHist = $state<number[]>([]);
  let gpuHist = $state<number[]>([]);
  let shell: HTMLDivElement;
  let maxHeight = $state(900);
  let tempStatus = $state<CpuTempStatus | null>(null);
  let tempBusy = $state(false);
  /** CPU temperature can be turned on but isn't yet: show the setup button. */
  let tempSetup = $derived(tempStatus?.supported && !tempStatus.enabled ? tempStatus : null);

  // A newer version found by the updater (Rust side checks on startup and every few hours).
  let updateVersion = $state<string | null>(null);
  let updateBusy = $state(false);
  let updateFailed = $state(false);
  let update = $derived(
    updateVersion ? { version: updateVersion, busy: updateBusy, failed: updateFailed, onclick: installUpdate } : null,
  );

  async function installUpdate() {
    updateBusy = true;
    updateFailed = false;
    try {
      // Downloads, installs and restarts the app.
      await invoke("install_update");
    } catch {
      updateFailed = true;
    } finally {
      updateBusy = false;
    }
  }

  /** Linux: the CPU power counter is there but root-only until the setup ran. */
  let powerSetup = $state(false);
  let powerBusy = $state(false);

  async function enablePower() {
    powerBusy = true;
    try {
      powerSetup = await invoke<boolean>("enable_cpu_power");
    } finally {
      powerBusy = false;
    }
  }

  async function enableTemp() {
    if (!tempStatus) return;
    if (!tempStatus.pawnio) {
      invoke("open_pawnio_site");
      return;
    }
    tempBusy = true;
    try {
      tempStatus = await invoke<CpuTempStatus>("set_cpu_temp", { enable: true });
    } finally {
      tempBusy = false;
    }
  }

  function loadTheme(): Theme {
    try {
      return localStorage.getItem("theme") === "light" ? "light" : "dark";
    } catch {
      return "dark";
    }
  }

  function push(list: number[], value: number): number[] {
    const next = list.length >= HISTORY ? list.slice(1) : list.slice();
    next.push(value);
    return next;
  }

  onMount(() => {
    const unlisten: Array<Promise<() => void>> = [
      listen<Stats>("stats", (e) => {
        stats = e.payload;
        cpuHist = push(cpuHist, e.payload.cpu.usage);
        gpuHist = push(gpuHist, e.payload.gpu?.usage ?? 0);
      }),
      listen<boolean>("on-top-changed", (e) => (onTop = e.payload)),
      listen<CpuTempStatus>("cpu-temp-changed", (e) => (tempStatus = e.payload)),
      listen<{ version: string }>("update-available", (e) => (updateVersion = e.payload.version)),
    ];
    invoke<{ version: string } | null>("update_status").then((u) => u && (updateVersion = u.version));
    invoke<boolean>("is_on_top").then((v) => (onTop = v));
    invoke<CpuTempStatus>("cpu_temp_status").then((s) => (tempStatus = s));
    invoke<boolean>("cpu_power_setup").then((v) => (powerSetup = v));

    // The window always hugs the content, so it grows/shrinks with the view.
    // Sized in physical pixels using the WebView's own pixel ratio, which can
    // differ from the window's scale factor on mixed-DPI multi-monitor setups.
    let last = "";
    // The window starts hidden (tauri.conf.json) so it never flashes white;
    // it is shown once the UI has rendered and the window has its final size.
    let shown = false;
    const fit = async () => {
      try {
        await resize();
      } finally {
        // Show the window even if a step above failed (Wayland, for one,
        // doesn't let apps read or set their window position).
        if (!shown) {
          shown = true;
          await win.show();
        }
      }
    };
    const resize = async () => {
      const dpr = window.devicePixelRatio || 1;
      const monitor = await currentMonitor().catch(() => null);
      if (monitor) {
        // Never taller than the monitor's work area (taskbar excluded).
        maxHeight = Math.floor(monitor.workArea.size.height / dpr) - 24;
      }
      const r = shell.getBoundingClientRect();
      const w = Math.ceil(r.width * dpr);
      const h = Math.ceil(r.height * dpr);
      const size = `${w}x${h}`;
      if (size === last) return;
      last = size;
      await win.setSize(new PhysicalSize(w, h));

      // Growing downwards past the bottom of the screen? Slide the window up.
      if (monitor) {
        const pos = await win.outerPosition().catch(() => null);
        const bottom = monitor.workArea.position.y + monitor.workArea.size.height;
        const margin = Math.round(12 * dpr);
        if (pos && pos.y + h > bottom - margin) {
          const y = Math.max(monitor.workArea.position.y + margin, bottom - margin - h);
          await win.setPosition(new PhysicalPosition(pos.x, y)).catch(() => {});
        }
      }
    };
    const ro = new ResizeObserver(() => void fit());
    ro.observe(shell);
    unlisten.push(win.onScaleChanged(() => requestAnimationFrame(() => ((last = ""), void fit()))));

    return () => {
      ro.disconnect();
      unlisten.forEach((p) => p.then((f) => f()));
    };
  });

  // Tell the backend what is on screen so it only collects that.
  $effect(() => {
    invoke("set_view", { expanded, panel: expanded ? panel : "none" });
  });

  $effect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem("theme", theme);
    } catch {}
    win.setTheme(theme);
  });

  function expand(p: Panel) {
    if (p !== "none") panel = p;
    expanded = true;
  }
  const hide = () => invoke("hide_to_tray");
</script>

<div class="shell pulsar-shell" class:expanded bind:this={shell} style:max-height="{maxHeight}px">
  {#if expanded}
    <Expanded
      {stats}
      {cpuHist}
      {gpuHist}
      bind:panel
      {theme}
      {onTop}
      oncollapse={() => (expanded = false)}
      ontheme={() => (theme = theme === "dark" ? "light" : "dark")}
      ontop={() => invoke("set_on_top", { on: !onTop })}
      onhide={hide}
      {tempSetup}
      {tempBusy}
      onenabletemp={enableTemp}
      {powerSetup}
      {powerBusy}
      onenablepower={enablePower}
      {update}
    />
  {:else}
    <Compact {stats} onexpand={expand} onhide={hide} {tempSetup} {tempBusy} onenabletemp={enableTemp} {update} />
  {/if}
</div>

<style>
  .shell {
    width: 372px;
    background: var(--card);
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: thin;
  }
  .shell.expanded {
    width: 440px;
  }
</style>
