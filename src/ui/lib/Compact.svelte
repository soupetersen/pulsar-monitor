<script lang="ts">
  import type { CpuTempStatus, Panel, Stats } from "./types";
  import { f1, GB, pct } from "./format";
  import { t } from "./i18n";
  import TempSetup from "./TempSetup.svelte";
  import UpdateButton from "./UpdateButton.svelte";
  import type { UpdateState } from "./types";

  let {
    stats,
    onexpand,
    onhide,
    tempSetup,
    tempBusy,
    onenabletemp,
    update,
  }: {
    stats: Stats | null;
    onexpand: (p: Panel) => void;
    onhide: () => void;
    tempSetup: CpuTempStatus | null;
    tempBusy: boolean;
    onenabletemp: () => void;
    update: UpdateState | null;
  } = $props();

  const C = 2 * Math.PI * 18;

  interface Ring {
    panel: Panel;
    value: number;
    label: string;
    title: string;
  }

  let rings = $derived.by((): Ring[] => {
    if (!stats) return [];
    const { cpu, ram, gpu } = stats;
    const cpuLabel =
      cpu.tempC != null ? `CPU · ${Math.round(cpu.tempC)}°C` : cpu.powerW != null ? `CPU · ${Math.round(cpu.powerW)} W` : "CPU";
    const list: Ring[] = [{ panel: "cpu", value: cpu.usage, label: cpuLabel, title: t.viewProcesses.cpu }];
    if (gpu) {
      list.push({
        panel: "gpu",
        value: gpu.usage,
        label: gpu.tempC != null ? `GPU · ${gpu.tempC}°C` : "GPU",
        title: t.viewProcesses.gpu,
      });
    }
    list.push({
      panel: "ram",
      value: (ram.used / ram.total) * 100,
      label: `RAM · ${f1(ram.used / GB)} GB`,
      title: t.viewProcesses.ram,
    });
    if (gpu?.vramUsed != null && gpu.vramTotal) {
      list.push({
        panel: "gpu",
        value: (gpu.vramUsed / gpu.vramTotal) * 100,
        label: `VRAM · ${f1(gpu.vramUsed / GB)} GB`,
        title: t.viewProcesses.gpu,
      });
    }
    return list;
  });
</script>

<div class="card">
  <header data-tauri-drag-region>
    <span class="host"><i class="dot"></i>{stats?.host ?? "…"}</span>
    <div class="actions">
      {#if update}
        <UpdateButton {...update} />
      {/if}
      {#if tempSetup}
        <TempSetup status={tempSetup} busy={tempBusy} onclick={onenabletemp} />
      {/if}
      <button class="icon-btn" aria-label={t.expand} title={t.expand} onclick={() => onexpand("none")}>
        <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9" /><polyline points="9 21 3 21 3 15" /><line x1="21" y1="3" x2="14" y2="10" /><line x1="3" y1="21" x2="10" y2="14" /></svg>
      </button>
      <button class="icon-btn" aria-label={t.minimizeToTray} title={t.minimizeToTray} onclick={onhide}>
        <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="5" y1="12" x2="19" y2="12" /></svg>
      </button>
    </div>
  </header>

  {#if !stats}
    <div class="rings" style:grid-template-columns="repeat(4, minmax(0, 1fr))" aria-busy="true" aria-label={t.collecting}>
      {#each ["CPU", "GPU", "RAM", "VRAM"] as label (label)}
        <div class="ring placeholder">
          <span class="dial">
            <svg class="spinner" aria-hidden="true" width="52" height="52" viewBox="0 0 44 44">
              <circle cx="22" cy="22" r="18" fill="none" stroke="var(--track)" stroke-width="4" />
              <circle cx="22" cy="22" r="18" fill="none" stroke="var(--accent)" stroke-width="4" stroke-linecap="round" stroke-dasharray="22 113.1" />
            </svg>
          </span>
          <span class="label">{label}</span>
        </div>
      {/each}
    </div>
  {:else}
  <div class="rings" style:grid-template-columns="repeat({Math.max(rings.length, 1)}, minmax(0, 1fr))">
    {#each rings as r, i (i)}
      <button class="ring" title={r.title} aria-label="{r.label}: {pct(r.value)}. {r.title}" onclick={() => onexpand(r.panel)}>
        <span class="dial">
          <svg aria-hidden="true" width="52" height="52" viewBox="0 0 44 44">
            <circle cx="22" cy="22" r="18" fill="none" stroke="var(--track)" stroke-width="4" />
            <circle
              class="arc"
              cx="22"
              cy="22"
              r="18"
              fill="none"
              stroke="var(--accent)"
              stroke-width="4"
              stroke-linecap="round"
              transform="rotate(-90 22 22)"
              style:stroke-dasharray="{(Math.min(r.value, 100) / 100) * C} {C}"
            />
          </svg>
          <span class="value">{pct(r.value)}</span>
        </span>
        <span class="label">{r.label}</span>
      </button>
    {/each}
  </div>
  {/if}
</div>

<style>
  .card {
    padding: 8px 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 2px;
  }
  .host {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }
  .actions {
    display: flex;
    gap: 2px;
  }
  .rings {
    display: grid;
    gap: 4px;
  }
  .ring {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 6px 0 8px;
    border-radius: 10px;
    transition: background-color 0.15s ease, transform 0.2s ease;
  }
  .ring:hover {
    background: var(--hover);
    transform: translateY(-2px);
  }
  .dial {
    position: relative;
    width: 52px;
    height: 52px;
  }
  .ring.placeholder:hover {
    background: transparent;
    transform: none;
  }
  /* Only runs for the fraction of a second before the first sample arrives. */
  .spinner {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .arc {
    transition: stroke-dasharray var(--value-anim);
  }
  .value {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .label {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
</style>
