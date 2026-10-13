<script lang="ts">
  // Remove a device (an admin's action): the computers of the cluster agree, then let it in no more.
  import { ConnectError } from '@connectrpc/connect';
  import { Dialog } from 'bits-ui';
  import { Button, Status } from '@omnarc/ui';
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

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-black/60" />
    <Dialog.Content
      class="fixed top-1/2 left-1/2 z-50 grid w-[min(440px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 gap-4 rounded-2xl border border-line-2 bg-surface-1 p-6 text-text-1 shadow-2xl"
    >
      <Dialog.Title class="m-0 text-20 font-semibold">Remove {device?.name || 'this device'}?</Dialog.Title>
      <Dialog.Description class="m-0 text-14 text-text-2">
        It leaves the cluster: the other devices let it in no more. To come back it needs a new invitation.
      </Dialog.Description>
      {#if failed}<div role="alert"><Status kind="critical" label={failed} /></div>{/if}
      <div class="flex flex-wrap justify-end gap-2">
        <Button label="Cancel" onclick={() => (open = false)} />
        <Button variant="primary" label={busy ? 'Removing…' : 'Remove'} disabled={busy} onclick={remove} />
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
