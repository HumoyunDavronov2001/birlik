<script lang="ts">
  import Status from '../lib/components/Status.svelte';
  import Table, { type Column } from '../lib/components/Table.svelte';

  interface Device {
    id: string;
    name: string;
    system: string;
    load: number | null;
    link: string;
    online: boolean;
  }

  // the name, the load and the state sort; the table starts sorted by load, the busiest first
  const columns: Column<Device>[] = [
    { key: 'name', label: 'Device', sort: (row) => row.name },
    { key: 'system', label: 'System' },
    { key: 'load', label: 'Load', align: 'end', mono: true, sort: (row) => row.load ?? -1 },
    { key: 'link', label: 'Link from here' },
    { key: 'online', label: 'State', sort: (row) => (row.online ? 0 : 1) },
  ];
  const rows: Device[] = [
    { id: 'a', name: 'Desktop', system: 'Linux · 16 cores', load: 0.42, link: '—', online: true },
    { id: 'b', name: 'MacBook Pro', system: 'macOS · 12 cores', load: 0.18, link: 'Wi-Fi · 3.2 ms · 422 Mbit/s', online: true },
    { id: 'c', name: 'Pixel 9 Pro', system: 'Android · 8 cores', load: null, link: '—', online: false },
  ];
</script>

<div style="max-width: 760px; display: grid; gap: 24px">
  <Table caption="Devices" captionVisible {columns} {rows} key={(row) => row.id} order={{ key: 'load', ascending: false }}>
    {#snippet cell(row, column)}
      {#if column.key === 'online'}
        <Status kind={row.online ? 'good' : 'warning'} label={row.online ? 'Online' : 'Offline'} />
      {:else if column.key === 'load'}
        {row.load === null ? '—' : `${Math.round(row.load * 100)}%`}
      {:else}
        {String(row[column.key as keyof Device])}
      {/if}
    {/snippet}
  </Table>
  <Table caption="Models" captionVisible columns={[{ key: 'name', label: 'Model' }]} rows={[]} key={() => ''} empty="No models on this device yet" />
</div>
