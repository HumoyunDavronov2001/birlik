/** Ring geometry shared by the Ring view and the logo. Angles are in degrees, clockwise from the top. */

export interface Arc {
  /** Start angle. */
  a0: number;
  /** End angle. */
  a1: number;
  /** Middle of the slot the arc sits in (gap included). */
  mid: number;
}

/** Splits the circle into arcs proportional to `shares`, leaving `gap` degrees between neighbours. */
export function layoutArcs(shares: readonly number[], gap = 1.6): Arc[] {
  const total = shares.reduce((a, b) => a + b, 0);
  if (total <= 0) return shares.map(() => ({ a0: 0, a1: 0, mid: 0 }));
  let angle = 0;
  return shares.map((share) => {
    const slot = (share / total) * 360;
    const arc = { a0: angle + gap / 2, a1: angle + slot - gap / 2, mid: angle + slot / 2 };
    angle += slot;
    return arc;
  });
}

export function point(cx: number, cy: number, r: number, deg: number): [number, number] {
  const t = ((deg - 90) * Math.PI) / 180;
  return [cx + r * Math.cos(t), cy + r * Math.sin(t)];
}

/** SVG path for an arc of radius `r` from `a0` to `a1`. */
export function arcPath(cx: number, cy: number, r: number, a0: number, a1: number): string {
  const [x0, y0] = point(cx, cy, r, a0);
  const [x1, y1] = point(cx, cy, r, a1);
  const large = a1 - a0 > 180 ? 1 : 0;
  return `M${x0.toFixed(2)} ${y0.toFixed(2)}A${r} ${r} 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
}

/** Gap (degrees) that keeps round stroke caps of `width` from touching, plus `visible` degrees of air. */
export function capGap(r: number, width: number, visible = 7): number {
  return ((2 * (width / 2)) / r) * (180 / Math.PI) + visible;
}

/** Load level 1–7 for a busy fraction 0–1. */
export function loadLevel(busy: number): number {
  return Math.min(7, Math.max(1, Math.round(1 + busy * 6)));
}
