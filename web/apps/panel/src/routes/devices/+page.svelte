<script lang="ts">
  import { Button, Card, Icon, Status } from '@omnarc/ui';
  import RemoveDevice from '$lib/RemoveDevice.svelte';
  import { cluster } from '$lib/cluster.svelte';
  import type { DeviceInfo } from '$lib/gen/omnarc/v1/device_pb';
  import { deviceName, formatGib, formatLink, isOnline, linkTo, platformIcon, platformName, short } from '$lib/model';

  const view = $derived(cluster.view);
  const ids = $derived(Object.keys(view.devices));

  // the device an admin is about to remove
  let removing = $state<DeviceInfo | null>(null);
  let confirming = $state(false);
</script>

<svelte:head><title>Devices · {view.name || 'Omnarc'}</title></svelte:head>

<Card title="Devices">
  <div class="overflow-x-auto">
    <table class="w-full border-collapse text-13">
      <thead>
        <tr class="text-left text-12 text-text-3">
          <th class="py-2 pr-3 font-normal">Device</th>
          <th class="py-2 pr-3 font-normal">System</th>
          <th class="py-2 pr-3 text-right font-normal">CPU</th>
          <th class="py-2 pr-3 text-right font-normal">Memory</th>
          <th class="py-2 pr-3 font-normal">Link from this device</th>
          <th class="py-2 pr-3 font-normal">State</th>
          <th class="py-2 pr-3 font-normal">Id</th>
          {#if view.canInvite}<th class="py-2 font-normal"><span class="sr-only">Actions</span></th>{/if}
        </tr>
      </thead>
      <tbody>
        {#each ids as id (id)}
          {@const device = view.devices[id]}
          {@const telemetry = view.telemetry[id]}
          {@const online = isOnline(view, id, cluster.now)}
          {@const link = linkTo(view, id)}
          <tr class="border-t border-line">
            <td class="py-2.5 pr-3">
              <span class="flex items-center gap-2.5">
                <span class="text-text-3"><Icon name={platformIcon(device.platform)} size={18} /></span>
                <span>{deviceName(view, id)}{#if id === view.thisNode}<span class="text-text-3"> · this device</span>{/if}</span>
              </span>
            </td>
            <td class="py-2.5 pr-3 text-text-2">{platformName(device.platform)}{device.cpuCores ? ` · ${device.cpuCores} cores` : ''}</td>
            <td class="py-2.5 pr-3 text-right font-mono text-text-2">{online && telemetry ? `${Math.round(telemetry.cpuLoad * 100)}%` : '—'}</td>
            <td class="py-2.5 pr-3 text-right font-mono text-text-2">
              {online && telemetry ? `${formatGib(telemetry.memoryUsedBytes)} / ` : ''}{device.memoryTotalBytes ? formatGib(device.memoryTotalBytes) : '—'}
            </td>
            <td class="py-2.5 pr-3 whitespace-nowrap text-text-2">{online && link ? formatLink(link) : '—'}</td>
            <td class="py-2.5 pr-3"><Status kind={online ? 'good' : 'warning'} label={online ? 'Online' : 'Offline'} /></td>
            <td class="py-2.5 pr-3 font-mono text-12 text-text-3">{short(id)}</td>
            {#if view.canInvite}
              <td class="py-2.5 text-right">
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
              </td>
            {/if}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</Card>

<RemoveDevice device={removing} bind:open={confirming} />
