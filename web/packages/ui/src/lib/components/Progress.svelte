<script lang="ts">
  // How far a task got: a download, a fetch, a benchmark. Without a value it shows that work goes on. The bar moves by
  // transform only.
  interface Props {
    label: string;
    /** Done so far, 0 … max; none: not known yet. */
    value?: number;
    max?: number;
    /** The text at the end, such as "1.2 of 4.9 GB"; the default is a percentage. */
    detail?: string;
  }

  let { label, value, max = 1, detail }: Props = $props();
  const share = $derived(value === undefined ? undefined : Math.min(1, Math.max(0, value / max)));
  const shown = $derived(detail ?? (share === undefined ? '' : `${Math.round(share * 100)}%`));
</script>

<div class="om-progress">
  <div class="head">
    <span class="label">{label}</span>
    {#if shown}<span class="detail">{shown}</span>{/if}
  </div>
  <div
    class="track"
    role="progressbar"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={max}
    aria-valuenow={value}
    aria-valuetext={shown || undefined}
  >
    <span class="fill" class:indeterminate={share === undefined} style:transform={share === undefined ? undefined : `scaleX(${share})`}></span>
  </div>
</div>

<style>
  .om-progress {
    display: grid;
    gap: 6px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 12px;
  }
  .label {
    color: var(--text-2);
  }
  .detail {
    font-family: var(--mono);
    color: var(--text-3);
  }
  .track {
    position: relative;
    height: 6px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }
  .fill {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--turquoise);
    transform-origin: left center;
    transition: transform 320ms var(--ease);
  }
  .indeterminate {
    width: 40%;
    animation: om-progress 1.4s var(--ease) infinite;
  }
  /* without motion an indeterminate bar must not look like a value: it fills the track, faintly */
  @media (prefers-reduced-motion: reduce) {
    .indeterminate {
      width: 100%;
      opacity: 0.35;
      animation: none;
    }
  }
  @keyframes om-progress {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(250%);
    }
  }
</style>
