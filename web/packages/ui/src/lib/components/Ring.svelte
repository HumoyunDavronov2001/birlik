<script lang="ts">
  import { arcPath, layoutArcs, point, type Arc } from '../geometry';
  import { ICONS } from '../icons';

  /** A thin outer marker on a device's arc (PLAN.md §9.4): charging, the battery, the temperature — always an icon. */
  export interface RingMarker {
    icon: 'bolt' | 'battery' | 'temp';
    kind: 'good' | 'warning' | 'serious' | 'critical';
    /** Said with the arc and shown on hover, e.g. "Charging · 81%" or "Hot". */
    label: string;
  }

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
    markers?: RingMarker[];
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
    /**
     * Opens a device (Enter or a click on its arc). With it, the arcs take keyboard focus: the arrow keys move from arc
     * to arc, Home and End go to the first and the last.
     */
    onselect?: (index: number) => void;
  }

  let { devices, label, hops = [], variant = 'full', highlight = null, center, tokens = 0, onselect }: Props = $props();

  // the arc that holds the keyboard focus (one tab stop for the whole ring)
  let active = $state(0);
  let focused = $state(-1);
  const arcEls: SVGPathElement[] = $state([]);

  function onkeydown(event: KeyboardEvent, i: number) {
    const n = devices.length;
    const next: Record<string, number> = {
      ArrowRight: (i + 1) % n,
      ArrowDown: (i + 1) % n,
      ArrowLeft: (i - 1 + n) % n,
      ArrowUp: (i - 1 + n) % n,
      Home: 0,
      End: n - 1,
    };
    if (event.key in next) {
      event.preventDefault();
      active = next[event.key];
      arcEls[active]?.focus();
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onselect?.(i);
    }
  }

  function arcLabel(i: number): string {
    const marks = devices[i].markers ?? [];
    const said = `${devices[i].name}: ${devices[i].note ?? `${devices[i].layers} layers, ${stageMs[i]} ms`}`;
    return marks.length ? `${said}; ${marks.map((m) => m.label).join(', ')}` : said;
  }

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

  // the markers run just outside the arcs; a badge holds each one's icon
  const markerR = $derived(box.r + box.width / 2 + (full ? 7 : 5));
  const badgeR = $derived(full ? 10 : 8);
  const iconSize = $derived(full ? 13 : 11);
  const SEVERITY = ['good', 'warning', 'serious', 'critical'] as const;
  const worst = (marks: RingMarker[]) =>
    marks.reduce((a, b) => (SEVERITY.indexOf(b.kind) > SEVERITY.indexOf(a.kind) ? b : a)).kind;

  // badges sit near the clockwise end of an arc, clear of the name at its middle; on a short arc, at its middle
  function badgeAngles(arc: Arc, n: number): number[] {
    const step = ((badgeR * 2 + 3) / markerR) * (180 / Math.PI);
    if (arc.a1 - arc.a0 >= step * (n + 1.5)) return Array.from({ length: n }, (_, k) => arc.a1 - step / 2 - 1 - k * step);
    return Array.from({ length: n }, (_, k) => arc.mid + (k - (n - 1) / 2) * step);
  }

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

  // a device with markers starts its leader line beyond them
  function labelPos(mid: number, marked: boolean) {
    const rad = (mid * Math.PI) / 180;
    const [x, y] = point(box.cx, box.cy, box.r + box.width / 2 + 18, mid);
    const [x2, y2] = point(box.cx, box.cy, marked ? markerR + 4 : box.r + box.width / 2 + 4, mid);
    const anchor = Math.abs(Math.sin(rad)) < 0.2 ? 'middle' : Math.sin(rad) >= 0 ? 'start' : 'end';
    const dy = Math.cos(rad) > 0.6 ? -14 : Math.cos(rad) < -0.6 ? 10 : 0;
    const tx = x + (anchor === 'start' ? 6 : anchor === 'end' ? -6 : 0);
    return { x, y, x2, y2, anchor, dy, tx };
  }
</script>

<svg class="om-ring" class:interactive={!!onselect} viewBox="0 0 {box.w} {box.h}" role={onselect ? 'group' : 'img'} aria-label={label}>
  <circle cx={box.cx} cy={box.cy} r={box.r} fill="none" stroke-width={box.width + 10} style:stroke="var(--surface-1)" />
  {#if focused >= 0 && arcs[focused]}
    <!-- the focused arc: a turquoise ring around it, as :focus-visible draws around a control -->
    <path
      d={arcPath(box.cx, box.cy, box.r, arcs[focused].a0, arcs[focused].a1)}
      fill="none"
      stroke-width={box.width + 8}
      class="focus"
    />
  {/if}
  {#each arcs as arc, i (i)}
    {#if onselect}
      <path
        bind:this={arcEls[i]}
        d={arcPath(box.cx, box.cy, box.r, arc.a0, arc.a1)}
        fill="none"
        stroke-width={box.width}
        style:stroke={stroke(i)}
        role="button"
        tabindex={i === active ? 0 : -1}
        aria-label={arcLabel(i)}
        onkeydown={(event) => onkeydown(event, i)}
        onclick={() => onselect?.(i)}
        onfocus={() => {
          active = i;
          focused = i;
        }}
        onblur={() => (focused = -1)}
      >
        <title>{arcLabel(i)}</title>
      </path>
    {:else}
      <path d={arcPath(box.cx, box.cy, box.r, arc.a0, arc.a1)} fill="none" stroke-width={box.width} style:stroke={stroke(i)}>
        <title>{arcLabel(i)}</title>
      </path>
    {/if}
  {/each}
  {#each arcs as arc, i (i)}
    {@const marks = devices[i].markers ?? []}
    {#if marks.length}
      <path
        d={arcPath(box.cx, box.cy, markerR, arc.a0, arc.a1)}
        fill="none"
        stroke-width={full ? 3 : 2.5}
        class="marker"
        style:stroke="var(--status-{worst(marks)})"
      />
      {#each badgeAngles(arc, marks.length) as angle, k (k)}
        {@const [x, y] = point(box.cx, box.cy, markerR, angle)}
        <g class="badge" style:color="var(--status-{marks[k].kind})" aria-hidden="true">
          <title>{marks[k].label}</title>
          <circle cx={x} cy={y} r={badgeR} />
          <svg x={x - iconSize / 2} y={y - iconSize / 2} width={iconSize} height={iconSize} viewBox="0 0 24 24">
            {@html ICONS[marks[k].icon]}
          </svg>
        </g>
      {/each}
    {/if}
  {/each}
  {#if full}
    {#each arcs as arc, i (i)}
      {@const p = labelPos(arc.mid, !!devices[i].markers?.length)}
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
  .interactive path[role='button'] {
    cursor: pointer;
    outline: none;
  }
  .focus {
    stroke: var(--turquoise);
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
  .marker {
    stroke-linecap: round;
  }
  .badge circle {
    fill: var(--surface-1);
    stroke: currentColor;
    stroke-width: 1.5;
  }
  .badge svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .halo {
    fill: var(--turquoise);
    opacity: 0.18;
  }
  .dot {
    fill: var(--turquoise);
  }
</style>
