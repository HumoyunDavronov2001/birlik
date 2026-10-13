<script lang="ts">
  import Status from '../lib/components/Status.svelte';
  import Table, { type Column } from '../lib/components/Table.svelte';

  interface Device {
    id: string;
    name: string;
    system: string;
    cpu: string;
    link: string;
    online: boolean;
  }

  const columns: Column[] = [
    { key: 'name', label: 'Device' },
    { key: 'system', label: 'System' },
    { key: 'cpu', label: 'CPU', align: 'end', mono: true },
    { key: 'link', label: 'Link from this device' },
    { key: 'online', label: 'State' },
  ];
  const rows: Device[] = [
    { id: 'a', name: 'Desktop', system: 'Linux · 16 cores', cpu: '42%', link: '—', online: true },
    { id: 'b', name: 'MacBook Pro', system: 'macOS · 12 cores', cpu: '18%', link: 'Wi-Fi · 3.2 ms · 422 Mbit/s', online: true },
    { id: 'c', name: 'Pixel 9 Pro', system: 'Android · 8 cores', cpu: '—', link: '—', online: false },
  ];
</script>

<div style="max-width: 760px; display: grid; gap: 24px">
  <Table caption="Devices" captionVisible {columns} {rows} key={(row) => row.id}>
    {#snippet cell(row, column)}
      {#if column.key === 'online'}
        <Status kind={row.online ? 'good' : 'warning'} label={row.online ? 'Online' : 'Offline'} />
      {:else}
        {String(row[column.key as keyof Device])}
      {/if}
    {/snippet}
  </Table>
  <Table caption="Models" captionVisible columns={[{ key: 'name', label: 'Model' }]} rows={[]} key={() => ''} empty="No models on this device yet" />
</div>
