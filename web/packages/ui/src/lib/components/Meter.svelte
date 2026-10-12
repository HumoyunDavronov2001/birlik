<script lang="ts">
  interface Props {
    /** 0–1. */
    value: number;
    /** Accessible name, e.g. "Memory". */
    label: string;
    /** A step of the load scale (1–7); without it the meter uses the accent color. */
    level?: number;
  }

  let { value, label, level }: Props = $props();
  const pct = $derived(Math.round(Math.min(1, Math.max(0, value)) * 100));
</script>

<div class="bk-meter" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
  <i style:width="{pct}%" style:background={level ? `var(--load-${level})` : undefined}></i>
</div>

<style>
  .bk-meter {
    height: 6px;
    min-width: 48px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }
  i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--turquoise);
  }
</style>
