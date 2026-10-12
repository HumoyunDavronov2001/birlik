<script lang="ts">
  // Add a device (PLAN.md §9.6, flow 2): a one-time ticket as a QR code and a command, then the four symbols that both
  // screens show, and the owner's answer.
  import { Dialog } from 'bits-ui';
  import { Button } from '@omnarc/ui';
  import { api, cluster } from './cluster.svelte';
  import { platformName } from './model';

  let { open = $bindable(false) }: { open?: boolean } = $props();

  let ticket = $state<{ text: string; svg: string; expires: number } | null>(null);
  let failed = $state(false);
  let answered = $state<{ name: string; joined: boolean } | null>(null);
  let copied = $state(false);

  const attempt = $derived(cluster.view.joinAttempt);
  const command = $derived(ticket ? `omnarc-node join ${ticket.text}` : '');
  const left = $derived(ticket ? Math.max(0, Math.round((ticket.expires - cluster.now) / 1000)) : 0);
  const countdown = $derived(`${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}`);

  // a fresh ticket each time the dialog opens; nothing is kept once it closes
  $effect(() => {
    if (!open) {
      ticket = null;
      failed = false;
      answered = null;
      copied = false;
      return;
    }
    if (ticket || attempt || answered) return;
    api.createTicket({ ttlSeconds: 600 }).then(
      (response) => {
        ticket = { text: response.qrPayload, svg: response.qrSvg, expires: Number(response.ticket?.expiresUnixMs ?? 0n) };
      },
      () => (failed = true),
    );
  });

  async function answer(matches: boolean) {
    if (!attempt) return;
    const name = attempt.device?.name || 'The device';
    await api.answerJoin({ joiner: attempt.joiner, matches });
    cluster.view.joinAttempt = null;
    ticket = null;
    answered = { name, joined: matches };
  }

  async function copy() {
    await navigator.clipboard.writeText(command);
    copied = true;
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-black/60" />
    <Dialog.Content
      class="fixed top-1/2 left-1/2 z-50 grid w-[min(520px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 gap-4 rounded-2xl border border-line-2 bg-surface-1 p-6 text-text-1 shadow-2xl"
    >
      <Dialog.Title class="m-0 text-20 font-semibold">Add a device</Dialog.Title>

      {#if attempt}
        <Dialog.Description class="m-0 text-14 text-text-2">
          {attempt.device?.name || 'A device'} ({platformName(attempt.device?.platform ?? 0)}) asks to join. Do these four
          symbols match the ones on its screen?
        </Dialog.Description>
        <ul class="m-0 grid list-none grid-cols-4 gap-2 p-0" aria-label="The symbols on this screen">
          {#each attempt.symbols as symbol, i (i)}
            <li class="grid justify-items-center gap-1 rounded-xl border border-line bg-surface-2 px-2 py-3">
              <span class="text-40 leading-none" aria-hidden="true">{symbol.emoji}</span>
              <span class="text-12 text-text-2">{symbol.name}</span>
            </li>
          {/each}
        </ul>
        <div class="flex flex-wrap justify-end gap-2">
          <Button label="No, they're different" onclick={() => answer(false)} />
          <Button variant="primary" label="Yes, they match" onclick={() => answer(true)} />
        </div>
      {:else if answered}
        <Dialog.Description class="m-0 text-14 text-text-2">
          {answered.joined
            ? `${answered.name} joined the cluster. It appears on the ring as soon as its service starts.`
            : `${answered.name} was refused, and its ticket no longer works.`}
        </Dialog.Description>
        <div class="flex justify-end"><Button variant="primary" label="Done" onclick={() => (open = false)} /></div>
      {:else if ticket}
        <Dialog.Description class="m-0 text-14 text-text-2">
          The new device scans this code — or, on a computer, run this command on it:
        </Dialog.Description>
        <div class="mx-auto w-[240px] rounded-xl bg-white p-2 [&_svg]:block [&_svg]:h-auto [&_svg]:w-full">
          <!-- eslint-disable-next-line svelte/no-at-html-tags -- the SVG comes from this device's own service -->
          {@html ticket.svg}
        </div>
        <div class="grid gap-2">
          <code class="max-h-24 overflow-auto rounded-lg border border-line bg-surface-2 px-3 py-2 font-mono text-12 break-all text-text-2">{command}</code>
          <div class="flex items-center justify-between gap-3">
            <span class="text-12 text-text-3" role="timer">Valid for {countdown}. Then both screens show four symbols to compare.</span>
            <Button size="sm" label={copied ? 'Copied' : 'Copy'} onclick={copy} />
          </div>
        </div>
      {:else if failed}
        <Dialog.Description class="m-0 text-14 text-text-2">
          This device could not open an invitation. Only the device that created the cluster can add devices.
        </Dialog.Description>
      {:else}
        <Dialog.Description class="m-0 text-14 text-text-2">Opening an invitation…</Dialog.Description>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
