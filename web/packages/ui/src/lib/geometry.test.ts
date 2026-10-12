import { describe, expect, it } from 'vitest';
import { arcPath, capGap, layoutArcs, loadLevel, point } from './geometry';

describe('layoutArcs', () => {
  it('splits the circle in proportion to the shares, with a gap between arcs', () => {
    const arcs = layoutArcs([24, 6, 8, 4, 4, 18], 2);
    const spans = arcs.map((a) => a.a1 - a.a0 + 2);
    expect(spans.reduce((a, b) => a + b, 0)).toBeCloseTo(360, 6);
    expect(spans[0] / spans[1]).toBeCloseTo(24 / 6, 6);
    expect(arcs[0].a0).toBe(1);
    expect(arcs.at(-1)!.a1).toBeCloseTo(359, 6);
  });

  it('returns empty arcs when there is nothing to split', () => {
    expect(layoutArcs([0, 0])).toEqual([
      { a0: 0, a1: 0, mid: 0 },
      { a0: 0, a1: 0, mid: 0 },
    ]);
  });
});

describe('point and arcPath', () => {
  it('measures angles clockwise from the top', () => {
    const [x, y] = point(0, 0, 10, 90);
    expect(x).toBeCloseTo(10, 6);
    expect(y).toBeCloseTo(0, 6);
  });

  it('uses the large-arc flag only for arcs longer than half a circle', () => {
    expect(arcPath(0, 0, 10, 0, 90)).toContain(' 0 0 1 ');
    expect(arcPath(0, 0, 10, 0, 200)).toContain(' 0 1 1 ');
  });
});

describe('capGap and loadLevel', () => {
  it('leaves room for both round caps', () => {
    expect(capGap(34, 12, 0)).toBeCloseTo((12 / 34) * (180 / Math.PI), 6);
  });

  it('maps a busy fraction onto the 7-step load scale', () => {
    expect([0, 0.5, 1, 2, -1].map(loadLevel)).toEqual([1, 4, 7, 7, 1]);
  });
});
