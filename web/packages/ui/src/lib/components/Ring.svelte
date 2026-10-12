<script lang="ts">
  import { arcPath, layoutArcs, point } from '../geometry';

  export interface RingDeviceView {
    name: string;
    /** Model layers held by the device; sets the arc length. */
    layers: number;
    /** Step of the load scale, 1–7; sets the arc color. */
    level: number;
    /** Compute time per token on this device, in ms; sets the token speed on the arc. */
    ms?: number;
    /** Extra drafting time per token, in ms. */
    draftMs?: number;
    /** The line under the name instead of layers and time, e.g. while no model runs: "8 GiB · 23% CPU". */
    note?: string;
  }

  interface Props {
    devices: RingDeviceView[];
    /** Accessible summary, e.g. "6 devices; the slowest stage is the Laptop at 14 ms". */
    label: string;
    /** Link latency from device i to i+1, in ms. */
    hops?: number[];
    /** "full" draws names around the ring; "compact" is the small ring used on phones. */
    variant?: 'full' | 'compact';
    /** Highlight one arc (the viewer's own device) and mute the others. */
    highlight?: number | null;
    center?: { value: string; unit?: string; caption?: string };
    /** Number of tokens travelling around the ring (0 for a still ring). */
    tokens?: number;
  }

  let { devices, label, hops = [], variant = 'full', highlight = null, center, tokens = 0 }: Props = $props();

  const full = $derived(variant === 'full');
  const box = $derived(full ? { w: 640, h: 492, cx: 320, cy: 236, r: 132, width: 24 } : { w: 240, h: 240, cx: 120, cy: 120, r: 92, width: 18 });
  const arcs = $derived(layoutArcs(devices.map((d) => d.layers)));
  const stageMs = $derived(devices.map((d) => (d.ms ?? 10) + (d.draftMs ?? 0)));
  const hopMs = $derived(devices.map((_, i) => hops[i] ?? 2));
  const cycle = $derived(stageMs.reduce((a, b) => a + b, 0) + hopMs.reduce((a, b) => a + b, 0));
  // the slowest stage is only marked when the ring runs a model and its stages report their time
  const slowest = $derived(devices.some((d) => d.ms !== undefined) ? stageMs.indexOf(Math.max(...stageMs)) : -1);
  // a long name would run past the edge of the drawing
  const shortName = (name: string) => (name.length > 18 ? `${name.slice(0, 17)}…` : name);

  // Token time -> angle: compute happens inside an arc, network time is spent crossing a gap.
  function angleAt(t: number): number {
    for (let i = 0; i < devices.length; i++) {
      const arc = arcs[i];
      if (t < stageMs[i]) return arc.a0 + (arc.a1 - arc.a0) * (t / stageMs[i]);
      t -= stageMs[i];
      if (t < hopMs[i]) {
        const next = arcs[(i + 1) % devices.length].a0 + (i === devices.length - 1 ? 360 : 0);
        return arc.a1 + (next - arc.a1) * (t / hopMs[i]);
      }
      t -= hopMs[i];
    }
    return 0;
  }

  let now = $state(1200);
  $effect(() => {
    if (tokens === 0 || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    let raf = requestAnimationFrame(function tick(t) {
      now = t;
      raf = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(raf);
  });
  const dots = $derived(
    Array.from({ length: tokens }, (_, k) => point(box.cx, box.cy, box.r, angleAt((now / 42 + (k * cycle) / tokens) % cycle))),
  );

  function stroke(i: number): string {
    if (highlight !== null) return i === highlight ? 'var(--turquoise)' : 'var(--load-2)';
    return `var(--load-${devices[i].level})`;
  }

  function labelPos(mid: number) {
    const rad = (mid * Math.PI) / 180;
    const [x, y] = point(box.cx, box.cy, box.r + box.width / 2 + 18, mid);
    const [x2, y2] = point(box.cx, box.cy, box.r + box.width / 2 + 4, mid);
    const anchor = Math.abs(Math.sin(rad)) < 0.2 ? 'middle' : Math.sin(rad) >= 0 ? 'start' : 'end';
    const dy = Math.cos(rad) > 0.6 ? -14 : Math.cos(rad) < -0.6 ? 10 : 0;
    const tx = x + (anchor === 'start' ? 6 : anchor === 'end' ? -6 : 0);
    return { x, y, x2, y2, anchor, dy, tx };
  }
</script>

<svg class="om-ring" viewBox="0 0 {box.w} {box.h}" role="img" aria-label={label}>
  <circle cx={box.cx} cy={box.cy} r={box.r} fill="none" stroke-width={box.width + 10} style:stroke="var(--surface-1)" />
  {#each arcs as arc, i (i)}
    <path d={arcPath(box.cx, box.cy, box.r, arc.a0, arc.a1)} fill="none" stroke-width={box.width} style:stroke={stroke(i)}>
      <title>{devices[i].name}: {devices[i].note ?? `${devices[i].layers} layers, ${stageMs[i]} ms`}</title>
    </path>
  {/each}
  {#if full}
    {#each arcs as arc, i (i)}
      {@const p = labelPos(arc.mid)}
      <line x1={p.x2} y1={p.y2} x2={p.x} y2={p.y} class="t-line" stroke-width="1" />
      <text x={p.tx} y={p.y + p.dy} text-anchor={p.anchor} class="t-name">{shortName(devices[i].name)}</text>
      <text x={p.tx} y={p.y + p.dy + 15} text-anchor={p.anchor} class="t-note"
        >{devices[i].note ?? `${devices[i].layers} layers · ${stageMs[i]} ms`}</text
      >
      {#if i === slowest}
        <text x={p.tx} y={p.y + p.dy + 30} text-anchor={p.anchor} class="t-bottleneck"
          ><tspan class="t-warn">▲</tspan> slowest stage</text
        >
      {/if}
    {/each}
  {/if}
  {#each dots as [x, y], k (k)}
    <circle cx={x} cy={y} r={Math.max(6, box.width * 0.55)} class="halo" />
    <circle cx={x} cy={y} r={Math.max(3, box.width * 0.26)} class="dot" />
  {/each}
  {#if center}
    <text x={box.cx} y={box.cy + (full ? 0 : 6)} text-anchor="middle" class={full ? 't-hero' : 't-name'}>{center.value}</text>
    {#if center.unit}<text x={box.cx} y={box.cy + (full ? 26 : 24)} text-anchor="middle" class="t-unit">{center.unit}</text>{/if}
    {#if center.caption && full}<text x={box.cx} y={box.cy + 56} text-anchor="middle" class="t-mono">{center.caption}</text>{/if}
  {/if}
</svg>

<style>
  .om-ring {
    display: block;
    width: 100%;
    height: auto;
  }
  .t-name {
    fill: var(--text-1);
    font: 500 13px var(--sans);
  }
  .t-note {
    fill: var(--text-3);
    font: 400 11px var(--sans);
  }
  .t-mono {
    fill: var(--text-2);
    font: 400 11px var(--mono);
  }
  .t-hero {
    fill: var(--text-1);
    font: 300 56px var(--sans);
    letter-spacing: -0.03em;
  }
  .t-unit {
    fill: var(--text-2);
    font: 400 14px var(--sans);
  }
  /* The label stays in a text color; only the marker carries the status color. */
  .t-bottleneck {
    fill: var(--text-2);
    font: 500 11px var(--sans);
  }
  .t-warn {
    fill: var(--status-warning);
  }
  .t-line {
    stroke: var(--line-2);
  }
  .halo {
    fill: var(--turquoise);
    opacity: 0.18;
  }
  .dot {
    fill: var(--turquoise);
  }
</style>
