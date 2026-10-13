<script lang="ts">
  // Remove a device (an admin's action): the computers of the cluster agree, then let it in no more.
  import { ConnectError } from '@connectrpc/connect';
  import { Button, Dialog, Status } from '@omnarc/ui';
  import { api } from './cluster.svelte';
  import type { DeviceInfo } from './gen/omnarc/v1/device_pb';

  let { device, open = $bindable(false) }: { device: DeviceInfo | null; open?: boolean } = $props();

  let busy = $state(false);
  let failed = $state('');

  $effect(() => {
    if (!open) {
      busy = false;
      failed = '';
    }
  });

  async function remove() {
    if (!device?.id) return;
    busy = true;
    failed = '';
    try {
      await api.removeMember({ member: device.id });
      open = false;
    } catch (e) {
      failed = e instanceof ConnectError ? e.rawMessage : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog bind:open size="sm" title="Remove {device?.name || 'this device'}?">
  <p class="m-0 text-14 text-text-2">
    It leaves the cluster: the other devices let it in no more. To come back it needs a new invitation.
  </p>
  {#if failed}<div role="alert"><Status kind="critical" label={failed} /></div>{/if}
  {#snippet actions()}
    <Button label="Cancel" onclick={() => (open = false)} />
    <Button variant="primary" label={busy ? 'Removing…' : 'Remove'} disabled={busy} onclick={remove} />
  {/snippet}
</Dialog>
