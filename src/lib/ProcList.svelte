<script lang="ts">
  import type { ProcRow } from "./types";
  import { t } from "./i18n";

  let { rows }: { rows: ProcRow[] | null } = $props();
</script>

<div class="list">
  {#if rows === null}
    <p class="empty muted">{t.loading}</p>
  {:else if rows.length === 0}
    <p class="empty muted">{t.noProcesses}</p>
  {:else}
    {#each rows as r (r.name)}
      <div class="row">
        <div class="names">
          <span class="name">{r.name}</span>
          {#if r.sub}<span class="sub">{r.sub}</span>{/if}
        </div>
        <div class="bar"><div class="fill" style:width="{r.w}%"></div></div>
        <span class="val">{r.val}</span>
      </div>
    {/each}
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 96px 58px;
    gap: 10px;
    align-items: center;
    padding: 5px 6px;
    border-radius: 6px;
    transition: background-color 0.15s ease;
  }
  .row:hover {
    background: var(--hover);
  }
  .names {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    font-size: 12px;
  }
  .sub {
    font-size: 11px;
    color: var(--muted);
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--track);
  }
  .fill {
    height: 4px;
    border-radius: 2px;
    background: var(--accent);
    transition: width var(--value-anim);
  }
  .val {
    font-size: 12px;
    font-weight: 600;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .empty {
    margin: 0;
    padding: 8px 6px;
    font-size: 12px;
  }
</style>
