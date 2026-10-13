import { create } from '@bufbuild/protobuf';
import { describe, expect, it } from 'vitest';
import {
  ClusterSnapshotSchema,
  JoinAttemptSchema,
  WatchClusterResponseSchema,
} from './gen/omnarc/v1/cluster_service_pb';
import { DeviceInfoSchema, NodeIdSchema, Platform } from './gen/omnarc/v1/device_pb';
import { LinkKind, LinkSampleSchema, PowerStateSchema, TelemetrySchema, ThermalState } from './gen/omnarc/v1/telemetry_pb';
import {
  apply,
  deviceName,
  formatGib,
  formatLink,
  fromSnapshot,
  heardAgo,
  hex,
  isOnline,
  linkTo,
  ONLINE_MS,
  platformName,
  power,
  thermal,
} from './model';

const id = (byte: number) => create(NodeIdSchema, { ed25519PublicKey: new Uint8Array(32).fill(byte) });

describe('the cluster view', () => {
  const snapshot = create(ClusterSnapshotSchema, {
    name: 'Home cluster',
    thisNode: id(1),
    canInvite: true,
    devices: [create(DeviceInfoSchema, { id: id(1), name: 'Desktop', platform: Platform.LINUX, memoryTotalBytes: 16n << 30n })],
    telemetry: [create(TelemetrySchema, { node: id(1), cpuLoad: 0.25 })],
  });

  it('starts from a snapshot', () => {
    const view = fromSnapshot(snapshot, 1_000);
    expect(view.name).toBe('Home cluster');
    expect(view.thisNode).toBe(hex(id(1)));
    expect(deviceName(view, hex(id(1)))).toBe('Desktop');
    expect(isOnline(view, hex(id(1)), 1_000 + ONLINE_MS - 1)).toBe(true);
    expect(isOnline(view, hex(id(1)), 1_000 + ONLINE_MS)).toBe(false);
  });

  it('follows the live stream', () => {
    let view = fromSnapshot(snapshot, 0);
    const joined = create(DeviceInfoSchema, { id: id(2), name: 'Laptop', platform: Platform.WINDOWS });
    view = apply(view, create(WatchClusterResponseSchema, { event: { case: 'deviceJoined', value: joined } }), 10);
    expect(deviceName(view, hex(id(2)))).toBe('Laptop');
    expect(isOnline(view, hex(id(2)), 10)).toBe(false);

    view = apply(view, create(WatchClusterResponseSchema, { event: { case: 'telemetry', value: create(TelemetrySchema, { node: id(2), cpuLoad: 0.5 }) } }), 20);
    expect(isOnline(view, hex(id(2)), 20)).toBe(true);
    expect(view.telemetry[hex(id(2))].cpuLoad).toBe(0.5);

    const attempt = create(JoinAttemptSchema, { joiner: id(3) });
    view = apply(view, create(WatchClusterResponseSchema, { event: { case: 'joinAttempt', value: attempt } }), 30);
    expect(view.joinAttempt).not.toBeNull();
    // once the device has joined, it is no longer waiting
    const phone = create(DeviceInfoSchema, { id: id(3), name: 'Phone' });
    view = apply(view, create(WatchClusterResponseSchema, { event: { case: 'deviceJoined', value: phone } }), 40);
    expect(view.joinAttempt).toBeNull();

    view = apply(view, create(WatchClusterResponseSchema, { event: { case: 'deviceLeft', value: id(2) } }), 50);
    expect(isOnline(view, hex(id(2)), 50)).toBe(false);
  });

  it('names what it shows', () => {
    expect(formatGib(16n << 30n)).toBe('16 GiB');
    expect(formatGib(5.1 * 2 ** 30)).toBe('5.1 GiB');
    expect(platformName(Platform.MACOS)).toBe('macOS');
    const view = fromSnapshot(snapshot, 0);
    expect(deviceName(view, 'abcdef0123456789')).toBe('abcdef0123');
  });
});

describe('links', () => {
  it('reads this device\'s link to another from its own telemetry', () => {
    const link = create(LinkSampleSchema, { peer: id(2), kind: LinkKind.WIFI, rttMs: 3.24, bandwidthMbps: 421.6 });
    const view = fromSnapshot(
      create(ClusterSnapshotSchema, { thisNode: id(1), telemetry: [create(TelemetrySchema, { node: id(1), links: [link] })] }),
      0,
    );
    expect(linkTo(view, hex(id(2)))).toBe(view.telemetry[hex(id(1))].links[0]);
    expect(linkTo(view, hex(id(1)))).toBeUndefined();
    expect(linkTo(view, hex(id(3)))).toBeUndefined();
    expect(formatLink(link)).toBe('Wi-Fi · 3.2 ms · 422 Mbit/s');
    expect(formatLink(create(LinkSampleSchema, { kind: LinkKind.ETHERNET, rttMs: 41, bandwidthMbps: 2350 }))).toBe(
      'Ethernet · 41 ms · 2.4 Gbit/s',
    );
    expect(formatLink(create(LinkSampleSchema, { rttMs: 0.4 }))).toBe('0.4 ms');
  });
});

describe("a device's state", () => {
  it('says how warm a device is, where its power comes from and when it was heard', () => {
    expect(thermal(ThermalState.NOMINAL)).toEqual({ label: 'Normal', kind: 'good' });
    expect(thermal(ThermalState.SERIOUS)?.kind).toBe('serious');
    expect(thermal(ThermalState.CRITICAL)).toEqual({ label: 'Too hot', kind: 'critical' });
    // a device that cannot read its temperature says nothing about it
    expect(thermal(ThermalState.UNSPECIFIED)).toBeNull();
    expect(thermal(undefined)).toBeNull();

    expect(power(create(PowerStateSchema, { charging: true, batteryFraction: 0.974 }))).toBe('Charging · 97%');
    expect(power(create(PowerStateSchema, { onBattery: true, batteryFraction: 0.5 }))).toBe('On battery · 50%');
    expect(power(create(PowerStateSchema, {}))).toBe('Plugged in');
    expect(power(undefined)).toBeNull();

    const view = fromSnapshot(create(ClusterSnapshotSchema, { telemetry: [create(TelemetrySchema, { node: id(1) })] }), 1_000);
    expect(heardAgo(view, hex(id(1)), 4_400)).toBe(3);
    expect(heardAgo(view, 'never', 4_400)).toBeNull();
  });
});
