<script lang="ts">
  import { slide } from "svelte/transition";
  import ProcList from "./ProcList.svelte";
  import Sparkline from "./Sparkline.svelte";
  import type { CpuTempStatus, Panel, Proc, ProcRow, Stats, Theme } from "./types";
  import { bytes, f1, f2, GB, pct, rate, shortCpuName, uptime } from "./format";
  import { t } from "./i18n";
  import TempSetup from "./TempSetup.svelte";

  let {
    stats,
    cpuHist,
    gpuHist,
    panel = $bindable(),
    theme,
    onTop,
    oncollapse,
    ontheme,
    ontop,
    onhide,
    tempSetup,
    tempBusy,
    onenabletemp,
  }: {
    stats: Stats | null;
    cpuHist: number[];
    gpuHist: number[];
    panel: Panel;
    theme: Theme;
    onTop: boolean;
    oncollapse: () => void;
    ontheme: () => void;
    ontop: () => void;
    onhide: () => void;
    tempSetup: CpuTempStatus | null;
    tempBusy: boolean;
    onenabletemp: () => void;
  } = $props();

  const togglePanel = (p: Panel) => (panel = panel === p ? "none" : p);

  function rows(kind: Panel): ProcRow[] | null {
    if (!stats || stats.procsFor !== kind || !stats.procs) return null;
    const total = stats.ram.total;
    const metric = (p: Proc) => (kind === "cpu" ? p.cpu : kind === "gpu" ? Math.min(p.gpu, 100) : p.mem);
    const max = Math.max(...stats.procs.map(metric), 1e-6);
    return stats.procs.map((p) => {
      const many = p.count > 1 ? t.processes(p.count) : "";
      let sub: string[];
      let val: string;
      if (kind === "cpu") {
        sub = [many, bytes(p.mem)];
        val = `${f1(p.cpu)}%`;
      } else if (kind === "gpu") {
        sub = [p.vram > 0 ? `${bytes(p.vram)} VRAM` : "", many];
        val = pct(Math.min(p.gpu, 100));
      } else {
        sub = [t.ofRam(f1((p.mem / total) * 100)), many];
        val = bytes(p.mem);
      }
      return { name: p.name, sub: sub.filter(Boolean).join(" · "), val, w: Math.max(3, (metric(p) / max) * 100) };
    });
  }

  let ramPct = $derived(stats ? (stats.ram.used / stats.ram.total) * 100 : 0);
</script>

{#snippet procsToggle(kind: "cpu" | "gpu" | "ram")}
  <button class="toggle" aria-expanded={panel === kind} onclick={() => togglePanel(kind)}>
    <span>{panel === kind ? t.hideProcs[kind] : t.showProcs[kind]}</span>
    <svg class="chev" class:open={panel === kind} aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
  </button>
  {#if panel === kind}
    <div transition:slide={{ duration: 180 }}>
      <ProcList rows={rows(kind)} />
    </div>
  {/if}
{/snippet}

<div class="card">
  <header data-tauri-drag-region>
    <div class="title">
      <span class="h">{t.system}</span>
      <span class="muted small">{stats ? t.uptime(uptime(stats.uptimeSecs)) : "…"}</span>
    </div>
    <div class="actions">
      <button class="icon-btn" aria-label={theme === "dark" ? t.toLight : t.toDark} title={t.changeTheme} onclick={ontheme}>
        {#if theme === "dark"}
          <svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4" /><line x1="12" y1="2" x2="12" y2="4" /><line x1="12" y1="20" x2="12" y2="22" /><line x1="4.9" y1="4.9" x2="6.3" y2="6.3" /><line x1="17.7" y1="17.7" x2="19.1" y2="19.1" /><line x1="2" y1="12" x2="4" y2="12" /><line x1="20" y1="12" x2="22" y2="12" /><line x1="4.9" y1="19.1" x2="6.3" y2="17.7" /><line x1="17.7" y1="6.3" x2="19.1" y2="4.9" /></svg>
        {:else}
          <svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" /></svg>
        {/if}
      </button>
      <button class="icon-btn" class:active={onTop} aria-pressed={onTop} aria-label={t.alwaysOnTop} title={t.alwaysOnTop} onclick={ontop}>
        <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22" /><path d="M5 17h14v-1.8a2 2 0 0 0-1.1-1.8l-1.8-.9A2 2 0 0 1 15 10.8V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.8a2 2 0 0 1-1.1 1.8l-1.8.9A2 2 0 0 0 5 15.2Z" /></svg>
      </button>
      <button class="icon-btn" aria-label={t.collapseAria} title={t.collapse} onclick={oncollapse}>
        <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 14 10 14 10 20" /><polyline points="20 10 14 10 14 4" /><line x1="14" y1="10" x2="21" y2="3" /><line x1="3" y1="21" x2="10" y2="14" /></svg>
      </button>
      <button class="icon-btn" aria-label={t.minimizeToTray} title={t.minimizeToTray} onclick={onhide}>
        <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="5" y1="12" x2="19" y2="12" /></svg>
      </button>
    </div>
  </header>

  {#if stats}
    {@const c = stats.cpu}
    <section>
      <div class="head">
        <span>CPU · {shortCpuName(c.name)}</span>
      </div>
      <div class="big-row">
        <span class="big">{pct(c.usage)}</span>
        <Sparkline values={cpuHist} />
      </div>
      {#if c.cores.length}
        <div class="threads" title={t.perThread}>
          {#each c.cores as core, i (i)}
            <div class="thread" style:height="{Math.max(8, core)}%"></div>
          {/each}
        </div>
      {/if}
      <div class="stats">
        <div>
          <span class="muted small">{t.temp}</span>
          {#if c.tempC != null}
            <b>{Math.round(c.tempC)}°C</b>
          {:else if tempSetup}
            <TempSetup status={tempSetup} busy={tempBusy} onclick={onenabletemp} label />
          {:else}
            <b>—</b>
          {/if}
        </div>
        <div><span class="muted small">{t.power}</span><b>{c.powerW != null ? `${Math.round(c.powerW)} W` : "—"}</b></div>
        <div><span class="muted small">Clock</span><b>{c.clockGhz ? `${f2(c.clockGhz)} GHz` : "—"}</b></div>
        <div><span class="muted small">Threads</span><b>{c.threads}</b></div>
      </div>
      {@render procsToggle("cpu")}
    </section>

    {#if stats.gpu}
      {@const g = stats.gpu}
      <section>
        <div class="head">
          <span>GPU · {g.name}</span>
          {#if g.clockMhz != null}<span>{g.clockMhz} MHz</span>{/if}
        </div>
        <div class="big-row">
          <span class="big">{pct(g.usage)}</span>
          <Sparkline values={gpuHist} />
        </div>
        <div class="stats">
          <div><span class="muted small">{t.temp}</span><b>{g.tempC != null ? `${g.tempC}°C` : "—"}</b></div>
          <div><span class="muted small">{t.power}</span><b>{g.powerW != null ? `${Math.round(g.powerW)} W` : "—"}</b></div>
          <div><span class="muted small">VRAM</span><b>{g.vramUsed != null && g.vramTotal ? `${f1(g.vramUsed / GB)} / ${Math.round(g.vramTotal / GB)} GB` : "—"}</b></div>
          <div><span class="muted small">{t.fan}</span><b>{g.fanPct != null ? `${g.fanPct}%` : "—"}</b></div>
        </div>
        {@render procsToggle("gpu")}
      </section>
    {/if}

    <section>
      <div class="head">
        <span>{t.memory}</span>
        <span>{f1(stats.ram.used / GB)} / {f1(stats.ram.total / GB)} GB</span>
      </div>
      <div class="big-row">
        <span class="big">{pct(ramPct)}</span>
        <div class="ram-bar">
          <div class="bar"><div class="fill" style:width="{ramPct}%"></div></div>
          <span class="muted small">{t.available(f1(stats.ram.available / GB))}</span>
        </div>
      </div>
      {@render procsToggle("ram")}
    </section>

    <div class="pair">
      <div class="mini">
        <span class="muted small">{t.network}</span>
        <b>{stats.net ? `↓ ${rate(stats.net.rxPerSec)} · ↑ ${rate(stats.net.txPerSec)}` : "—"}</b>
      </div>
      <div class="mini">
        <span class="muted small">{t.disk}</span>
        <b>{stats.disk ? `${pct(stats.disk.activePct)} · ${rate(stats.disk.bytesPerSec)}` : "—"}</b>
      </div>
    </div>
  {:else}
    <p class="muted small">{t.collecting}</p>
  {/if}
</div>

<style>
  .card {
    padding: 12px 14px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .title {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .h {
    font-size: 15px;
    font-weight: 600;
  }
  .small {
    font-size: 11px;
  }
  .actions {
    display: flex;
    gap: 2px;
  }
  .icon-btn.active {
    background: var(--chip);
    color: var(--accent);
  }
  section {
    padding: 12px;
    border-radius: 12px;
    background: var(--inset);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .big-row {
    display: flex;
    align-items: flex-end;
    gap: 14px;
  }
  .big {
    font-size: 30px;
    font-weight: 700;
    line-height: 1;
    min-width: 72px;
    font-variant-numeric: tabular-nums;
  }
  .threads {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 26px;
  }
  .thread {
    flex: 1;
    border-radius: 2px;
    background: var(--accent);
    opacity: 0.7;
    transition: height var(--value-anim);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }
  .stats > div,
  .mini {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .stats b,
  .mini b {
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .ram-bar {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .bar {
    height: 8px;
    border-radius: 4px;
    background: var(--track);
  }
  .fill {
    height: 8px;
    border-radius: 4px;
    background: var(--accent);
    transition: width var(--value-anim);
  }
  .toggle {
    height: 34px;
    padding: 0 10px;
    border-radius: 8px;
    background: var(--chip);
    font-size: 12px;
    font-weight: 500;
    display: flex;
    align-items: center;
    justify-content: space-between;
    transition: background-color 0.15s ease;
  }
  .toggle:hover {
    background: var(--hover);
  }
  .chev {
    transition: transform 0.22s ease;
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .pair {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .mini {
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--inset);
  }
</style>
