// The cluster as the panel keeps it, and the pure functions that change and describe it.

import type { ClusterSnapshot, JoinAttempt, WatchClusterResponse } from './gen/omnarc/v1/cluster_service_pb';
import { Platform, type DeviceInfo, type NodeId } from './gen/omnarc/v1/device_pb';
import { LinkKind, ThermalState, type LinkSample, type PowerState, type Telemetry } from './gen/omnarc/v1/telemetry_pb';

/** A device is online while its telemetry keeps arriving; it is sent every second. */
export const ONLINE_MS = 5_000;

export interface ClusterView {
  name: string;
  /** The cluster id (hex). */
  id: string;
  /** The node that serves this panel. */
  thisNode: string;
  canInvite: boolean;
  /** The cluster's relays, without their tokens. */
  relays: string[];
  /** Devices by node id (hex), in the order they became known. */
  devices: Record<string, DeviceInfo>;
  telemetry: Record<string, Telemetry>;
  /** When the panel last heard from each device (its own clock, ms). */
  heard: Record<string, number>;
  /** A device that asks to join and waits for the owner. */
  joinAttempt: JoinAttempt | null;
}

export function emptyView(): ClusterView {
  return {
    name: '',
    id: '',
    thisNode: '',
    canInvite: false,
    relays: [],
    devices: {},
    telemetry: {},
    heard: {},
    joinAttempt: null,
  };
}

export function hex(id?: NodeId): string {
  return id ? Array.from(id.ed25519PublicKey, (b) => b.toString(16).padStart(2, '0')).join('') : '';
}

export function short(id: string): string {
  return id.slice(0, 10);
}

/** The view after a snapshot. */
export function fromSnapshot(snapshot: ClusterSnapshot, now: number): ClusterView {
  const view = emptyView();
  view.name = snapshot.name;
  view.id = snapshot.id ? Array.from(snapshot.id.ed25519PublicKey, (b) => b.toString(16).padStart(2, '0')).join('') : '';
  view.thisNode = hex(snapshot.thisNode);
  view.canInvite = snapshot.canInvite;
  view.relays = [...snapshot.relays];
  for (const device of snapshot.devices) view.devices[hex(device.id)] = device;
  for (const telemetry of snapshot.telemetry) {
    const id = hex(telemetry.node);
    view.telemetry[id] = telemetry;
    view.heard[id] = now;
  }
  return view;
}

/** The view after one event of the live stream; the same object, changed. */
export function apply(view: ClusterView, response: WatchClusterResponse, now: number): ClusterView {
  const event = response.event;
  switch (event.case) {
    case 'snapshot':
      return fromSnapshot(event.value, now);
    case 'deviceJoined': {
      const id = hex(event.value.id);
      if (id) view.devices[id] = event.value;
      if (view.joinAttempt && hex(view.joinAttempt.joiner) === id) view.joinAttempt = null;
      break;
    }
    case 'deviceLeft':
      delete view.telemetry[hex(event.value)];
      delete view.heard[hex(event.value)];
      break;
    case 'deviceRemoved':
      delete view.devices[hex(event.value)];
      delete view.telemetry[hex(event.value)];
      delete view.heard[hex(event.value)];
      break;
    case 'telemetry': {
      const id = hex(event.value.node);
      view.telemetry[id] = event.value;
      view.heard[id] = now;
      break;
    }
    case 'joinAttempt':
      view.joinAttempt = event.value;
      break;
    default:
      break;
  }
  return view;
}

export function isOnline(view: ClusterView, id: string, now: number): boolean {
  return now - (view.heard[id] ?? -Infinity) < ONLINE_MS;
}

export function gib(bytes: bigint | number): number {
  return Number(bytes) / 2 ** 30;
}

export function formatGib(bytes: bigint | number): string {
  const value = gib(bytes);
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} GiB`;
}

export function platformName(platform: Platform): string {
  switch (platform) {
    case Platform.LINUX:
      return 'Linux';
    case Platform.WINDOWS:
      return 'Windows';
    case Platform.MACOS:
      return 'macOS';
    case Platform.ANDROID:
      return 'Android';
    case Platform.IOS:
      return 'iOS';
    case Platform.BROWSER:
      return 'Browser';
    default:
      return '—';
  }
}

export function platformIcon(platform: Platform): 'desktop' | 'laptop' | 'phone' | 'browser' {
  switch (platform) {
    case Platform.ANDROID:
    case Platform.IOS:
      return 'phone';
    case Platform.BROWSER:
      return 'browser';
    case Platform.MACOS:
      return 'laptop';
    default:
      return 'desktop';
  }
}

/** A device's name, or its short id before it has told the cluster what it is. */
export function deviceName(view: ClusterView, id: string): string {
  return view.devices[id]?.name || short(id);
}

/** This device's measured link to the device `id`, from this device's own telemetry. */
export function linkTo(view: ClusterView, id: string): LinkSample | undefined {
  if (id === view.thisNode) return undefined;
  return view.telemetry[view.thisNode]?.links.find((link) => hex(link.peer) === id);
}

/** A link as the panel shows it: `Wi-Fi · 3 ms · 420 Mbit/s`. */
export function formatLink(link: LinkSample): string {
  const kind: Partial<Record<LinkKind, string>> = {
    [LinkKind.WIFI]: 'Wi-Fi',
    [LinkKind.ETHERNET]: 'Ethernet',
    [LinkKind.USB]: 'USB',
    [LinkKind.THUNDERBOLT]: 'Thunderbolt',
    [LinkKind.RELAY]: 'Relay',
    [LinkKind.WEBRTC]: 'WebRTC',
  };
  const ms = link.rttMs < 10 ? link.rttMs.toFixed(1) : Math.round(link.rttMs).toString();
  const parts = [kind[link.kind], `${ms} ms`];
  if (link.bandwidthMbps > 0) {
    parts.push(link.bandwidthMbps >= 1000 ? `${(link.bandwidthMbps / 1000).toFixed(1)} Gbit/s` : `${Math.round(link.bandwidthMbps)} Mbit/s`);
  }
  return parts.filter(Boolean).join(' · ');
}

/** How warm a device runs, in words, with the status it is shown with; nothing when the device does not say. */
export function thermal(state: ThermalState | undefined): { label: string; kind: 'good' | 'warning' | 'serious' | 'critical' } | null {
  switch (state) {
    case ThermalState.NOMINAL:
      return { label: 'Normal', kind: 'good' };
    case ThermalState.FAIR:
      return { label: 'Warm', kind: 'warning' };
    case ThermalState.SERIOUS:
      return { label: 'Hot', kind: 'serious' };
    case ThermalState.CRITICAL:
      return { label: 'Too hot', kind: 'critical' };
    default:
      return null;
  }
}

/** Where a device's power comes from, in words; nothing when the device does not say. */
export function power(state: PowerState | undefined): string | null {
  if (!state) return null;
  const percent = `${Math.round(state.batteryFraction * 100)}%`;
  if (state.charging) return `Charging · ${percent}`;
  if (state.onBattery) return `On battery · ${percent}`;
  return 'Plugged in';
}

/** Seconds since the panel last heard from a device; nothing if never. */
export function heardAgo(view: ClusterView, id: string, now: number): number | null {
  const at = view.heard[id];
  return at === undefined ? null : Math.max(0, Math.round((now - at) / 1000));
}
