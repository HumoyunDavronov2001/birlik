// The cluster as the panel keeps it, and the pure functions that change and describe it.

import type { ClusterSnapshot, JoinAttempt, WatchClusterResponse } from './gen/omnarc/v1/cluster_service_pb';
import { Platform, type DeviceInfo, type NodeId } from './gen/omnarc/v1/device_pb';
import type { Telemetry } from './gen/omnarc/v1/telemetry_pb';

/** A device is online while its telemetry keeps arriving; it is sent every second. */
export const ONLINE_MS = 5_000;

export interface ClusterView {
  name: string;
  /** The cluster id (hex). */
  id: string;
  /** The node that serves this panel. */
  thisNode: string;
  canInvite: boolean;
  /** Devices by node id (hex), in the order they became known. */
  devices: Record<string, DeviceInfo>;
  telemetry: Record<string, Telemetry>;
  /** When the panel last heard from each device (its own clock, ms). */
  heard: Record<string, number>;
  /** A device that asks to join and waits for the owner. */
  joinAttempt: JoinAttempt | null;
}

export function emptyView(): ClusterView {
  return { name: '', id: '', thisNode: '', canInvite: false, devices: {}, telemetry: {}, heard: {}, joinAttempt: null };
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
