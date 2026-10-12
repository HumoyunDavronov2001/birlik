/** Types for living-ring.js — the Omnarc living ring (WebGL2). */
export interface RingDevice {
  /** Shown on the label, e.g. "Desktop". */
  name: string;
  /** Shown under the name, e.g. "CUDA". */
  backend: string;
  /** Relative compute power; layers are split in proportion to it. */
  power: number;
  /** Extra time factor for a slow device (default 1). */
  slowdown?: number;
}

export interface LivingRingOptions {
  devices: RingDevice[];
  /** Devices that addDevice() adds, in order (up to 8 devices in total). */
  reserve: RingDevice[];
  reducedMotion: boolean;
  /** Skip the intro scene (screenshots). */
  instant?: boolean;
  /** Element that receives pointer events (defaults to the canvas parent). */
  host?: HTMLElement;
  labelsEl?: HTMLElement | null;
  statusEl?: HTMLElement | null;
  statusText: (devices: number, layers: number) => string;
  labelText?: (device: RingDevice, layers: number) => string;
  isLight: () => boolean;
  cssVar: (name: string) => string;
  onContextLost?: () => void;
}

export interface LivingRing {
  /** Draws one frame; `ms` is a timestamp in milliseconds (e.g. from requestAnimationFrame). */
  frame(ms: number): void;
  /** Adds the next reserve device; returns false when no more can be added. */
  addDevice(): boolean;
  /** Back to the initial devices, replaying the intro. */
  reset(): void;
  /** Re-reads colors after a theme change. */
  setTheme(): void;
  /** Re-reads the canvas size. */
  resize(): void;
  /** Removes listeners and releases the GPU context. */
  destroy(): void;
  deviceCount(): number;
}

/** Returns null when WebGL2 is unavailable. */
export function livingRing(canvas: HTMLCanvasElement, options: LivingRingOptions): LivingRing | null;
