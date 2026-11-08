<script lang="ts">
  /** Values 0–100, oldest first. Drawn right-aligned so the line grows from the right. */
  let { values, slots = 30 }: { values: number[]; slots?: number } = $props();

  const W = 380;
  const H = 36;

  let points = $derived.by(() => {
    const step = W / (slots - 1);
    const n = values.length;
    return values
      .map((v, i) => `${(W - (n - 1 - i) * step).toFixed(1)},${(H - 2 - (Math.min(v, 100) / 100) * (H - 6)).toFixed(1)}`)
      .join(" ");
  });
  let area = $derived(values.length > 1 ? `${points.split(" ")[0].split(",")[0]},${H} ${points} ${W},${H}` : "");
</script>

<svg aria-hidden="true" viewBox="0 0 {W} {H}" preserveAspectRatio="none">
  {#if values.length > 1}
    <polygon points={area} fill="var(--accent)" fill-opacity="0.12" />
    <polyline points={points} fill="none" stroke="var(--accent)" stroke-width="2" stroke-linejoin="round" vector-effect="non-scaling-stroke" />
  {/if}
</svg>

<style>
  svg {
    flex: 1;
    min-width: 0;
    height: 36px;
  }
</style>
