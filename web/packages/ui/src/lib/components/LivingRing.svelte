<script lang="ts">
  import { onMount } from 'svelte';
  import { livingRing, type LivingRing, type RingDevice } from '../../../../../../design/living-ring.js';
  import Button from './Button.svelte';
  import Logo from './Logo.svelte';

  interface Props {
    devices: RingDevice[];
    /** Devices the "Add a device" button adds, in order. */
    reserve?: RingDevice[];
    controls?: boolean;
    label?: string;
    statusText?: (devices: number, layers: number) => string;
    /** The line under a device's name; by default its backend and its layers. */
    labelText?: (device: RingDevice, layers: number) => string;
    /** The names around the ring; a small ring in a dialog says them in its text instead. */
    labels?: boolean;
    addLabel?: string;
    resetLabel?: string;
    /** The ring formed at once, without the intro scene. */
    instant?: boolean;
  }

  let {
    devices,
    reserve = [],
    controls = true,
    label = 'The Omnarc living ring: every arc is a device, gold comets are tokens travelling around the ring',
    statusText = (n: number, layers: number) => `${n} devices · ${layers} layers · ${n} tokens in flight`,
    labelText,
    labels = true,
    addLabel = 'Add a device',
    resetLabel = 'Start over',
    instant = false,
  }: Props = $props();

  let host: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let labelsEl: HTMLDivElement;
  let statusEl: HTMLSpanElement | undefined = $state();
  let fallback = $state(false);
  let full = $state(false);
  let ring: LivingRing | null = null;
  const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

  onMount(() => {
    const root = document.documentElement;
    const reduced = reducedMotion();
    const isLight = () =>
      root.dataset.theme === 'light' || (!root.dataset.theme && matchMedia('(prefers-color-scheme: light)').matches);
    // Options are read once: the ring owns its state after it starts.
    const r = livingRing(canvas, {
      devices,
      reserve,
      reducedMotion: reduced,
      instant,
      host,
      labelsEl: labels ? labelsEl : null,
      statusEl,
      statusText,
      labelText,
      isLight,
      cssVar: (name) => getComputedStyle(root).getPropertyValue(name).trim(),
      onContextLost: () => (fallback = true),
    });
    if (!r) {
      fallback = true;
      return;
    }
    ring = r;

    let raf = 0;
    const draw = () => r.frame(performance.now());
    const loop = (t: number) => {
      r.frame(t);
      raf = requestAnimationFrame(loop);
    };
    const start = () => {
      if (!reduced && !raf) raf = requestAnimationFrame(loop);
    };
    const stop = () => {
      cancelAnimationFrame(raf);
      raf = 0;
    };
    const io = new IntersectionObserver(([entry]) => (entry.isIntersecting ? start() : stop()));
    io.observe(host);
    const ro = new ResizeObserver(() => {
      r.resize();
      if (reduced) draw();
    });
    ro.observe(canvas);
    const retheme = () => {
      r.setTheme();
      if (reduced) draw();
    };
    const mo = new MutationObserver(retheme);
    mo.observe(root, { attributes: true, attributeFilter: ['data-theme'] });
    const scheme = matchMedia('(prefers-color-scheme: light)');
    scheme.addEventListener('change', retheme);
    if (reduced) draw();
    else start();

    return () => {
      stop();
      io.disconnect();
      ro.disconnect();
      mo.disconnect();
      scheme.removeEventListener('change', retheme);
      r.destroy();
      ring = null;
    };
  });

  /** Grows the next reserve device's arc, as when a device joins; false when the ring holds no more. */
  export function add(): boolean {
    if (!ring) return false;
    const added = ring.addDevice();
    if (reducedMotion()) ring.frame(performance.now());
    return added;
  }

  function onAdd() {
    if (!ring) return;
    if (full) {
      ring.reset();
      full = false;
    } else {
      full = !ring.addDevice();
    }
    if (reducedMotion()) ring.frame(performance.now());
  }
</script>

<div class="om-living" bind:this={host}>
  <div class="stage" role="img" aria-label={label} hidden={fallback}>
    <canvas bind:this={canvas} aria-hidden="true"></canvas>
  </div>
  {#if fallback}
    <div class="fallback"><Logo size={320} title="Omnarc" /></div>
  {/if}
  <div class="labels" bind:this={labelsEl} aria-hidden="true" hidden={fallback || !labels}></div>
  {#if controls && !fallback}
    <div class="footer">
      <span class="status" bind:this={statusEl} aria-live="polite"></span>
      <Button size="sm" icon={full ? undefined : 'plus'} label={full ? resetLabel : addLabel} onclick={onAdd} />
    </div>
  {/if}
</div>

<style>
  .om-living {
    position: relative;
    width: min(var(--om-living-size, 560px), 100%);
    margin: 0 auto;
    touch-action: pan-y;
  }
  canvas {
    display: block;
    width: 100%;
    aspect-ratio: 1;
  }
  .stage[hidden],
  .labels[hidden] {
    display: none;
  }
  .fallback {
    display: grid;
    place-items: center;
    aspect-ratio: 1;
  }
  .labels {
    position: absolute;
    left: 0;
    top: 0;
    width: 0;
    height: 0;
    pointer-events: none;
  }
  .labels :global(span) {
    position: absolute;
    left: 0;
    top: 0;
    white-space: nowrap;
    font: 400 11px/1.35 var(--mono);
    color: var(--text-3);
    will-change: transform, opacity;
    opacity: 0;
  }
  .labels :global(span b) {
    display: block;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    font: 500 12px/1.3 var(--sans);
    color: var(--text-2);
  }
  @media (max-width: 560px) {
    .labels {
      display: none;
    }
  }
  .footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 10px 14px;
    margin-top: 4px;
  }
  .status {
    font: 400 12px var(--mono);
    color: var(--text-3);
  }
</style>
