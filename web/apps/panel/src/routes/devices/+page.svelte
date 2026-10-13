<script lang="ts">
  // Devices (PA-05): every device of the cluster in a table that sorts and filters. An offline device is greyed out
  // with the time it was last seen.
  import { Button, Card, Field, Icon, Status, Table, type Column } from '@omnarc/ui';
  import RemoveDevice from '$lib/RemoveDevice.svelte';
  import { cluster } from '$lib/cluster.svelte';
  import type { DeviceInfo } from '$lib/gen/omnarc/v1/device_pb';
  import {
    backends,
    deviceName,
    formatAgo,
    formatGib,
    formatLink,
    isOnline,
    linkTo,
    platformIcon,
    platformName,
    power,
    seenAgo,
    thermal,
  } from '$lib/model';

  const view = $derived(cluster.view);
  const ids = $derived(Object.keys(view.devices));
  const online = (id: string) => isOnline(view, id, cluster.now);
  // the system's name; its processor and cores are on the device's own page
  const system = (id: string) => platformName(view.devices[id].platform);

  // the filter looks in the name, the system and the backend
  let filter = $state('');
  const words = $derived(filter.trim().toLowerCase());
  const shown = $derived(
    words
      ? ids.filter((id) => `${deviceName(view, id)} ${system(id)} ${backends(view.devices[id])}`.toLowerCase().includes(words))
      : ids,
  );

  // a device on battery sorts first, the emptiest first; then the charging ones, then those plugged in
  function powerOrder(id: string): number {
    const state = online(id) ? view.telemetry[id]?.power : undefined;
    if (!state) return 3;
    if (state.charging) return 1 + state.batteryFraction;
    return state.onBattery ? state.batteryFraction : 2;
  }

  const columns = $derived<Column<string>[]>([
    { key: 'device', label: 'Device', wrap: true, sort: (id) => deviceName(view, id).toLowerCase() },
    { key: 'system', label: 'System', sort: system },
    { key: 'backend', label: 'Backend', sort: (id) => backends(view.devices[id]) },
    { key: 'cpu', label: 'Load', align: 'end', mono: true, sort: (id) => (online(id) ? (view.telemetry[id]?.cpuLoad ?? 0) : -1) },
    { key: 'memory', label: 'Memory', align: 'end', mono: true, sort: (id) => Number(view.devices[id].memoryTotalBytes) },
    { key: 'temperature', label: 'Temperature', sort: (id) => (online(id) ? (view.telemetry[id]?.thermal ?? 0) : -1) },
    { key: 'power', label: 'Power', sort: powerOrder },
    { key: 'link', label: 'Link from here', sort: (id) => (online(id) ? (linkTo(view, id)?.rttMs ?? 1e9) : 1e9) },
    // online first, then the offline ones, the most recently seen first
    { key: 'state', label: 'State', sort: (id) => (online(id) ? 0 : 1 + (seenAgo(view, id, cluster.now) ?? 1e12)) },
    ...(view.canInvite ? [{ key: 'actions', label: 'Actions', hideLabel: true }] : []),
  ]);

  // the device an admin is about to remove
  let removing = $state<DeviceInfo | null>(null);
  let confirming = $state(false);
</script>

<svelte:head><title>Devices · {view.name || 'Omnarc'}</title></svelte:head>

<Card title="Devices">
  <div class="flex flex-wrap items-end gap-x-4 gap-y-2">
    <Field label="Filter" type="search" bind:value={filter} placeholder="Name, system or backend" />
    <span class="pb-2.5 text-12 text-text-3" role="status">
      {words ? `${shown.length} of ${ids.length}` : `${ids.length} ${ids.length === 1 ? 'device' : 'devices'}`}
    </span>
  </div>
  <Table
    caption="The devices of {view.name || 'this cluster'}"
    {columns}
    rows={shown}
    key={(id) => id}
    empty={words ? `No device matches “${filter.trim()}”` : 'No devices yet'}
  >
    {#snippet cell(id, column)}
      {@const device = view.devices[id]}
      {@const telemetry = view.telemetry[id]}
      {@const up = online(id)}
      {#if column.key === 'device'}
        <a class="flex items-center gap-2.5 no-underline hover:underline {up ? 'text-text-1' : 'text-text-3'}" href="/devices/{id}">
          <span class="text-text-3"><Icon name={platformIcon(device.platform)} size={18} /></span>
          <span>{deviceName(view, id)}{#if id === view.thisNode}<span class="text-text-3"> · this device</span>{/if}</span>
        </a>
      {:else if column.key === 'system'}
        {system(id)}
      {:else if column.key === 'backend'}
        {backends(device)}
      {:else if column.key === 'cpu'}
        {up && telemetry ? `${Math.round(telemetry.cpuLoad * 100)}%` : '—'}
      {:else if column.key === 'memory'}
        {up && telemetry ? `${formatGib(telemetry.memoryUsedBytes)} / ` : ''}{device.memoryTotalBytes ? formatGib(device.memoryTotalBytes) : '—'}
      {:else if column.key === 'temperature'}
        {@const warmth = up ? thermal(telemetry?.thermal) : null}
        {#if warmth}<Status kind={warmth.kind} label={warmth.label} />{:else}—{/if}
      {:else if column.key === 'power'}
        {(up && power(telemetry?.power)) || '—'}
      {:else if column.key === 'link'}
        {@const link = linkTo(view, id)}
        {up && link ? formatLink(link) : '—'}
      {:else if column.key === 'state'}
        {@const ago = seenAgo(view, id, cluster.now)}
        <Status
          kind={up ? 'good' : 'warning'}
          label={up ? 'Online' : ago === null ? 'Not seen yet' : `Offline · ${formatAgo(ago)}`}
        />
      {:else if column.key === 'actions'}
        {#if id !== view.thisNode}
          <Button
            size="sm"
            variant="ghost"
            label="Remove"
            aria-label="Remove {deviceName(view, id)}"
            onclick={() => {
              removing = device;
              confirming = true;
            }}
          />
        {/if}
      {/if}
    {/snippet}
  </Table>
</Card>

<RemoveDevice device={removing} bind:open={confirming} />
