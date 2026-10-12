import { create } from '@bufbuild/protobuf';
import { describe, expect, it } from 'vitest';
import {
  ClusterSnapshotSchema,
  JoinAttemptSchema,
  WatchClusterResponseSchema,
} from './gen/omnarc/v1/cluster_service_pb';
import { DeviceInfoSchema, NodeIdSchema, Platform } from './gen/omnarc/v1/device_pb';
import { TelemetrySchema } from './gen/omnarc/v1/telemetry_pb';
import { apply, deviceName, formatGib, fromSnapshot, hex, isOnline, ONLINE_MS, platformName } from './model';

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
