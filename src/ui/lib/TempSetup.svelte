<script lang="ts">
  import type { CpuTempStatus } from "./types";
  import { t } from "./i18n";

  /** Shown while CPU temperature is supported but not enabled yet. */
  let {
    status,
    busy,
    onclick,
    label = false,
  }: { status: CpuTempStatus; busy: boolean; onclick: () => void; label?: boolean } = $props();

  let tip = $derived(busy ? t.cpuTempEnabling : status.pawnio ? t.cpuTempOff : t.cpuTempNeedsPawnio);
</script>

<button class="temp-setup" class:chip={label} class:busy disabled={busy} title={tip} aria-label={tip} {onclick}>
  <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M14 4v10.54a4 4 0 1 1-4 0V4a2 2 0 0 1 4 0Z" />
  </svg>
  <i class="dot"></i>
  {#if label}<span>{busy ? t.enabling : t.enable}</span>{/if}
</button>

<style>
  .temp-setup {
    position: relative;
    height: 30px;
    min-width: 30px;
    padding: 0 8px;
    border-radius: 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--muted);
    transition: background-color 0.15s ease, color 0.15s ease;
  }
  .temp-setup:hover {
    background: var(--hover);
    color: var(--text);
  }
  .temp-setup.chip {
    height: 24px;
    padding: 0 8px 0 6px;
    background: var(--chip);
    color: var(--text);
    font-size: 12px;
    font-weight: 600;
    align-self: flex-start;
  }
  .dot {
    position: absolute;
    top: 5px;
    left: 17px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .chip .dot {
    top: 3px;
    left: 15px;
  }
  .busy {
    cursor: progress;
  }
  .busy svg {
    animation: pulse 1.2s steps(6, end) infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
