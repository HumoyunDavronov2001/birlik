<script lang="ts">
  import { arcPath, capGap, layoutArcs, point } from '../geometry';

  interface Props {
    size?: number;
    /** "white" is for dark photos and the iOS Dynamic Island. */
    variant?: 'color' | 'white';
    /** 0–1: fills the arcs like the desktop tray icon. */
    load?: number | null;
    /** Accessible name; without it the logo is decorative. */
    title?: string;
  }

  let { size = 32, variant = 'color', load = null, title }: Props = $props();

  const SHARES = [0.29, 0.23, 0.18, 0.16, 0.14];
  const R = 34;
  const width = $derived(load === null ? 12 : 15);
  const arcs = $derived(layoutArcs(SHARES, capGap(R, width)));
  const dot = point(50, 50, R, 0); // the token sits in the top gap

  function stroke(i: number): string {
    if (variant === 'white') return '#f1f3f8';
    if (load !== null) return i / SHARES.length < load ? 'var(--turquoise)' : 'var(--surface-3)';
    return 'var(--turquoise)';
  }
</script>

<svg
  class="bk-logo"
  viewBox="0 0 100 100"
  width={size}
  height={size}
  role={title ? 'img' : undefined}
  aria-label={title}
  aria-hidden={title ? undefined : 'true'}
>
  {#each arcs as arc, i (i)}
    <path
      d={arcPath(50, 50, R, arc.a0, arc.a1)}
      fill="none"
      stroke-width={width}
      stroke-linecap="round"
      style:stroke={stroke(i)}
      style:opacity={variant === 'white' && i > 0 ? 0.55 : undefined}
    />
  {/each}
  <circle cx={dot[0]} cy={dot[1]} r="7.5" style:fill={variant === 'white' ? '#3dd7cf' : 'var(--gold-mark)'} />
</svg>

<style>
  .bk-logo {
    display: block;
    flex: none;
  }
</style>
