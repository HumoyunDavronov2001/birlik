<script lang="ts">
  // A device (PA-06): what it is, what it does now, how it reaches the others, and how the others reach it.
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { Button, Card, Chip, CodeBlock, EmptyState, Icon, Meter, Status, Table, loadLevel, type Column } from '@omnarc/ui';
  import RemoveDevice from '$lib/RemoveDevice.svelte';
  import { cluster } from '$lib/cluster.svelte';
  import type { LinkSample } from '$lib/gen/omnarc/v1/telemetry_pb';
  import {
    deviceName,
    formatAgo,
    formatGib,
    formatLink,
    gib,
    hex,
    isOnline,
    linkTo,
    platformIcon,
    platformName,
    power,
    seenAgo,
    thermal,
  } from '$lib/model';

  const view = $derived(cluster.view);
  const id = $derived(page.params.id ?? '');
  const device = $derived(view.devices[id]);
  const telemetry = $derived(view.telemetry[id]);
  const online = $derived(isOnline(view, id, cluster.now));
  const ago = $derived(seenAgo(view, id, cluster.now));
  const warmth = $derived(thermal(telemetry?.thermal));
  const supply = $derived(power(telemetry?.power));
  const memoryShare = $derived(
    telemetry && device?.memoryTotalBytes ? gib(telemetry.memoryUsedBytes) / gib(device.memoryTotalBytes) : 0,
  );
  const ownLink = $derived(linkTo(view, id));

  // how this device reaches each of the others, from its own measurements
  const linkColumns: Column[] = [
    { key: 'peer', label: 'To' },
    { key: 'link', label: 'Link' },
  ];
  const links = $derived<LinkSample[]>(online ? (telemetry?.links ?? []) : []);

  let confirming = $state(false);

  // once removed, the device is gone: back to the list
  let seen = false;
  $effect(() => {
    if (device) seen = true;
    else if (seen) void goto('/devices');
  });
</script>

<svelte:head><title>{device ? deviceName(view, id) : 'A device'} · {view.name || 'Omnarc'}</title></svelte:head>

<div class="grid gap-5">
  <a class="inline-flex w-fit items-center gap-1.5 text-13 text-text-2 no-underline hover:text-text-1" href="/devices">← Devices</a>

  {#if !device}
    <EmptyState icon="devices" title="No such device in this cluster" text="It may have been removed, or the panel has not heard of it yet." />
  {:else}
    <header class="flex flex-wrap items-center gap-3">
      <span class="grid size-12 place-items-center rounded-[10px] border border-line bg-surface-1 text-text-2">
        <Icon name={platformIcon(device.platform)} size={24} />
      </span>
      <div class="grid min-w-0 gap-1">
        <h2 class="m-0 text-28 font-semibold tracking-[-0.02em] wrap-anywhere">{deviceName(view, id)}</h2>
        <div class="flex flex-wrap gap-2">
          <Status kind={online ? 'good' : 'warning'} label={online ? 'Online' : ago === null ? 'Not seen yet' : `Offline · last seen ${formatAgo(ago)}`} />
          <!-- the system's own name says the platform already, as "Linux (Ubuntu 26.04)" -->
          <Chip label={device.osVersion || platformName(device.platform)} />
          {#if id === view.thisNode}<Chip label="This device" selected />{/if}
        </div>
      </div>
      {#if view.canInvite && id !== view.thisNode}
        <div class="ml-auto"><Button label="Remove" onclick={() => (confirming = true)} /></div>
      {/if}
    </header>

    <div class="grid gap-5 md:grid-cols-2">
      <Card title="Now">
        <dl class="m-0 grid gap-4 text-13">
          <div class="grid gap-1.5">
            <div class="flex justify-between"><dt class="text-text-2">Processor</dt><dd class="m-0 font-mono text-12">{online && telemetry ? `${Math.round(telemetry.cpuLoad * 100)}%` : '—'}</dd></div>
            <Meter value={online && telemetry ? telemetry.cpuLoad : 0} level={online && telemetry ? loadLevel(telemetry.cpuLoad) : undefined} label="Processor load" />
          </div>
          <div class="grid gap-1.5">
            <div class="flex justify-between">
              <dt class="text-text-2">Memory</dt>
              <dd class="m-0 font-mono text-12">
                {online && telemetry ? `${formatGib(telemetry.memoryUsedBytes)} / ` : ''}{device.memoryTotalBytes ? formatGib(device.memoryTotalBytes) : '—'}
              </dd>
            </div>
            <Meter value={online ? memoryShare : 0} label="Memory in use" />
          </div>
          <div class="flex items-center justify-between gap-3">
            <dt class="text-text-2">Temperature</dt>
            <dd class="m-0">{#if online && warmth}<Status kind={warmth.kind} label={warmth.label} />{:else}<span class="text-text-3">—</span>{/if}</dd>
          </div>
          <div class="flex items-center justify-between gap-3">
            <dt class="text-text-2">Power</dt>
            <dd class="m-0 text-text-1">{online && supply ? supply : '—'}</dd>
          </div>
        </dl>
      </Card>

      <Card title="Hardware">
        <dl class="m-0 grid gap-3 text-13">
          <div class="grid gap-1 sm:grid-cols-[120px_1fr]">
            <dt class="text-text-3">Processor</dt>
            <dd class="m-0">{device.cpuModel || '—'}{device.cpuCores ? ` · ${device.cpuCores} cores` : ''}</dd>
          </div>
          <div class="grid gap-1 sm:grid-cols-[120px_1fr]">
            <dt class="text-text-3">Memory</dt>
            <dd class="m-0">{device.memoryTotalBytes ? formatGib(device.memoryTotalBytes) : '—'}</dd>
          </div>
          <div class="grid gap-1 sm:grid-cols-[120px_1fr]">
            <dt class="text-text-3">Accelerators</dt>
            <dd class="m-0 grid gap-1">
              {#each device.accelerators as unit, i (i)}
                <span>{unit.name}{unit.memoryBytes ? ` · ${formatGib(unit.memoryBytes)}` : ''}</span>
              {:else}
                <span class="text-text-3">None that Omnarc can use — the processor computes</span>
              {/each}
            </dd>
          </div>
          <div class="grid gap-1 sm:grid-cols-[120px_1fr]">
            <dt class="text-text-3">Omnarc</dt>
            <dd class="m-0 font-mono text-12">{device.agentVersion || '—'}</dd>
          </div>
        </dl>
      </Card>

      <Card title="Links">
        {#if id !== view.thisNode}
          <p class="m-0 mb-3 text-13 text-text-2">
            From this device: <span class="text-text-1">{online && ownLink ? formatLink(ownLink) : 'not measured yet'}</span>
          </p>
        {/if}
        <Table caption="How {deviceName(view, id)} reaches the others" columns={linkColumns} rows={links} key={(link) => hex(link.peer)} empty={online ? 'No links measured yet' : 'The device is offline'}>
          {#snippet cell(link, column)}
            {#if column.key === 'peer'}
              {deviceName(view, hex(link.peer))}
            {:else}
              {formatLink(link)}
            {/if}
          {/snippet}
        </Table>
      </Card>

      <Card title="Reach">
        <div class="grid gap-3 text-13">
          <div class="grid gap-1.5">
            <span class="text-text-3">Node id</span>
            <CodeBlock code={id} copyLabel="Copy the node id" lines={2} />
          </div>
          <div class="grid gap-1">
            <span class="text-text-3">Addresses</span>
            {#each device.directAddresses as address (address)}
              <span class="font-mono text-12 text-text-2">{address}</span>
            {:else}
              <span class="text-text-3">None announced yet</span>
            {/each}
          </div>
          <div class="grid gap-1">
            <span class="text-text-3">Relay</span>
            <span class="font-mono text-12 break-all text-text-2">{device.relayUrl || 'None: reached on shared networks only'}</span>
          </div>
        </div>
      </Card>
    </div>
  {/if}
</div>

<RemoveDevice device={device ?? null} bind:open={confirming} />
