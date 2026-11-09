<script lang="ts">
  import { t } from "./i18n";

  /** Shown when a newer version is available; installs it and restarts. */
  let { version, busy, failed, onclick }: { version: string; busy: boolean; failed: boolean; onclick: () => void } =
    $props();

  let tip = $derived(busy ? t.updating : failed ? t.updateFailed : t.updateAvailable(version));
</script>

<button class="update" class:busy disabled={busy} title={tip} aria-label={tip} {onclick}>
  <svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M12 3v12" />
    <polyline points="7 10 12 15 17 10" />
    <path d="M5 21h14" />
  </svg>
  <i class="dot"></i>
</button>

<style>
  .update {
    position: relative;
    width: 30px;
    height: 30px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    color: var(--accent);
    transition: background-color 0.15s ease;
  }
  .update:hover {
    background: var(--hover);
  }
  .dot {
    position: absolute;
    top: 5px;
    right: 5px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .busy {
    cursor: progress;
  }
  .busy svg {
    animation: drop 1.2s steps(6, end) infinite;
  }
  @keyframes drop {
    50% {
      transform: translateY(2px);
      opacity: 0.5;
    }
  }
</style>
