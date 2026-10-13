<script lang="ts">
  import { Button, Card, Icon, Status, Table, type Column } from '@omnarc/ui';
  import RemoveDevice from '$lib/RemoveDevice.svelte';
  import { cluster } from '$lib/cluster.svelte';
  import type { DeviceInfo } from '$lib/gen/omnarc/v1/device_pb';
  import { deviceName, formatGib, formatLink, isOnline, linkTo, platformIcon, platformName, short } from '$lib/model';

  const view = $derived(cluster.view);
  const ids = $derived(Object.keys(view.devices));

  const columns = $derived<Column[]>([
    { key: 'device', label: 'Device' },
    { key: 'system', label: 'System' },
    { key: 'cpu', label: 'CPU', align: 'end', mono: true },
    { key: 'memory', label: 'Memory', align: 'end', mono: true },
    { key: 'link', label: 'Link from this device' },
    { key: 'state', label: 'State' },
    { key: 'id', label: 'Id', mono: true },
    ...(view.canInvite ? [{ key: 'actions', label: 'Actions', hideLabel: true }] : []),
  ]);

  // the device an admin is about to remove
  let removing = $state<DeviceInfo | null>(null);
  let confirming = $state(false);
</script>

<svelte:head><title>Devices · {view.name || 'Omnarc'}</title></svelte:head>

<Card title="Devices">
  <Table caption="The devices of {view.name || 'this cluster'}" {columns} rows={ids} key={(id) => id} empty="No devices yet">
    {#snippet cell(id, column)}
      {@const device = view.devices[id]}
      {@const telemetry = view.telemetry[id]}
      {@const online = isOnline(view, id, cluster.now)}
      {#if column.key === 'device'}
        <a class="flex items-center gap-2.5 text-text-1 no-underline hover:underline" href="/devices/{id}">
          <span class="text-text-3"><Icon name={platformIcon(device.platform)} size={18} /></span>
          <span>{deviceName(view, id)}{#if id === view.thisNode}<span class="text-text-3"> · this device</span>{/if}</span>
        </a>
      {:else if column.key === 'system'}
        {platformName(device.platform)}{device.cpuCores ? ` · ${device.cpuCores} cores` : ''}
      {:else if column.key === 'cpu'}
        {online && telemetry ? `${Math.round(telemetry.cpuLoad * 100)}%` : '—'}
      {:else if column.key === 'memory'}
        {online && telemetry ? `${formatGib(telemetry.memoryUsedBytes)} / ` : ''}{device.memoryTotalBytes ? formatGib(device.memoryTotalBytes) : '—'}
      {:else if column.key === 'link'}
        {@const link = linkTo(view, id)}
        <span class="whitespace-nowrap">{online && link ? formatLink(link) : '—'}</span>
      {:else if column.key === 'state'}
        <Status kind={online ? 'good' : 'warning'} label={online ? 'Online' : 'Offline'} />
      {:else if column.key === 'id'}
        <span class="text-text-3">{short(id)}</span>
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
