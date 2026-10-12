<script lang="ts">
  import { Card, Icon, Meter, Ring, loadLevel } from '@omnarc/ui';
  import { cluster } from '$lib/cluster.svelte';
  import { deviceName, formatGib, gib, isOnline, platformIcon, platformName } from '$lib/model';

  const view = $derived(cluster.view);

  // a phone gets the small ring without labels
  let narrow = $state(matchMedia('(max-width: 640px)').matches);
  $effect(() => {
    const query = matchMedia('(max-width: 640px)');
    const change = () => (narrow = query.matches);
    query.addEventListener('change', change);
    return () => query.removeEventListener('change', change);
  });
  const ids = $derived(Object.keys(view.devices));
  const totalBytes = $derived(ids.reduce((sum, id) => sum + Number(view.devices[id].memoryTotalBytes), 0));
  const usedBytes = $derived(ids.reduce((sum, id) => sum + Number(view.telemetry[id]?.memoryUsedBytes ?? 0n), 0));

  // until a model runs, an arc's length is the device's share of the cluster's memory
  const arcs = $derived(
    ids.map((id) => {
      const online = isOnline(view, id, cluster.now);
      const load = view.telemetry[id]?.cpuLoad ?? 0;
      return {
        name: deviceName(view, id),
        layers: Math.max(1, Math.round(gib(view.devices[id].memoryTotalBytes) * 10)),
        level: online ? loadLevel(load) : 1,
        note: `${formatGib(view.devices[id].memoryTotalBytes)} · ${online ? `${Math.round(load * 100)}% CPU` : 'offline'}`,
      };
    }),
  );
  const summary = $derived(
    `${ids.length} ${ids.length === 1 ? 'device' : 'devices'} with ${formatGib(totalBytes)} of memory together; ` +
      arcs.map((arc, i) => `${arc.name} ${isOnline(view, ids[i], cluster.now) ? 'online' : 'offline'}`).join(', '),
  );
</script>

<svelte:head><title>Ring · {view.name || 'Omnarc'}</title></svelte:head>

<div class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_380px]">
  <Card padding="lg">
    {#if ids.length}
      <Ring
        variant={narrow ? 'compact' : 'full'}
        devices={arcs}
        label={summary}
        center={{ value: String(ids.length), unit: ids.length === 1 ? 'device' : 'devices', caption: `${formatGib(totalBytes)} of memory` }}
      />
      <p class="m-0 text-center text-13 text-text-3">
        Every arc is a device, as long as its share of the cluster's memory; its color is its load. No model is running yet.
      </p>
    {:else}
      <p class="m-0 text-13 text-text-3">Waiting for this device's service…</p>
    {/if}
  </Card>

  <div class="grid content-start gap-5">
    <Card title="Together">
      <dl class="m-0 grid grid-cols-1 gap-4 min-[400px]:grid-cols-2">
        <div>
          <dt class="text-12 text-text-3">Memory in use</dt>
          <dd class="m-0 mt-1 font-mono text-20 whitespace-nowrap">
            {formatGib(usedBytes)} <span class="text-13 text-text-3">/ {formatGib(totalBytes)}</span>
          </dd>
        </div>
        <div>
          <dt class="text-12 text-text-3">Online</dt>
          <dd class="m-0 mt-1 font-mono text-20">
            {ids.filter((id) => isOnline(view, id, cluster.now)).length} <span class="text-13 text-text-3">/ {ids.length}</span>
          </dd>
        </div>
      </dl>
    </Card>

    <Card title="Devices">
      <ul class="m-0 grid list-none gap-1 p-0">
        {#each ids as id (id)}
          {@const device = view.devices[id]}
          {@const telemetry = view.telemetry[id]}
          {@const online = isOnline(view, id, cluster.now)}
          <li class="grid grid-cols-[20px_minmax(0,1fr)_72px_44px] items-center gap-3 py-2">
            <span class="text-text-3"><Icon name={platformIcon(device.platform)} size={18} /></span>
            <span class="min-w-0">
              <span class="block truncate text-13 text-text-1">
                {deviceName(view, id)}{#if id === view.thisNode}<span class="text-text-3"> · this device</span>{/if}
              </span>
              <span class="block text-12 text-text-3">{platformName(device.platform)} · {online ? 'online' : 'offline'}</span>
            </span>
            <Meter value={online ? (telemetry?.cpuLoad ?? 0) : 0} label={`CPU of ${deviceName(view, id)}`} level={online ? loadLevel(telemetry?.cpuLoad ?? 0) : 1} />
            <span class="text-right font-mono text-12 text-text-2">{online ? `${Math.round((telemetry?.cpuLoad ?? 0) * 100)}%` : '—'}</span>
          </li>
        {/each}
      </ul>
    </Card>
  </div>
</div>
