<script lang="ts">
  // Add a device (PLAN.md §9.6, flow 2): a one-time ticket as a QR code and a command, then the four symbols that both
  // screens show, and the owner's answer.
  import { ConnectError } from '@connectrpc/connect';
  import { Button, CodeBlock, Dialog, EmojiCheck, LivingRing, Progress, QrCode, Status, toasts } from '@omnarc/ui';
  import { api, cluster } from './cluster.svelte';
  import { gib, hex, platformName } from './model';

  let { open = $bindable(false) }: { open?: boolean } = $props();

  let ticket = $state<{ text: string; svg: string; expires: number } | null>(null);
  let failed = $state(false);
  let answered = $state<{ id: string; name: string; joined: boolean; platform: number; memory: bigint } | null>(null);
  // the moment a device joins: the ring as it was, then the new arc grows (PLAN.md §9.3)
  let living: { add(): boolean } | undefined = $state();
  const ringBefore = $derived(
    Object.entries(cluster.view.devices)
      .filter(([id]) => id !== answered?.id)
      .map(([, d]) => ({
        name: d.name || 'A device',
        backend: platformName(d.platform),
        power: Math.max(1, gib(d.memoryTotalBytes)),
      })),
  );
  $effect(() => {
    if (answered?.joined && living) {
      const grow = setTimeout(() => living?.add(), 400);
      return () => clearTimeout(grow);
    }
  });

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
    // read before the answer travels: the device may join meanwhile, and its attempt is gone before the reply comes
    const asked = attempt;
    if (!asked) return;
    const joiner = {
      id: hex(asked.joiner),
      name: asked.device?.name || 'The device',
      platform: asked.device?.platform ?? 0,
      memory: asked.device?.memoryTotalBytes ?? 0n,
    };
    if (matches) cluster.expected.add(joiner.id);
    try {
      await api.answerJoin({ joiner: asked.joiner, matches });
    } catch (e) {
      cluster.expected.delete(joiner.id);
      const reason = e instanceof ConnectError ? e.rawMessage : String(e);
      toasts.show({ kind: 'critical', title: 'The answer did not reach the device', body: reason }, 0);
      return;
    }
    cluster.view.joinAttempt = null;
    ticket = null;
    answered = { ...joiner, joined: matches };
  }
</script>

<Dialog bind:open title="Add a device">
  {#if attempt}
    <p class="m-0 text-14 text-text-2">
      {attempt.device?.name || 'A device'} ({platformName(attempt.device?.platform ?? 0)}) asks to join. Do these four symbols
      match the ones on its screen?
    </p>
    <EmojiCheck symbols={attempt.symbols} />
  {:else if answered}
    {#if answered.joined}
      <div class="[--om-living-size:300px]">
        <LivingRing
          bind:this={living}
          devices={ringBefore}
          reserve={[{ name: answered.name, backend: platformName(answered.platform), power: Math.max(1, gib(answered.memory)) }]}
          controls={false}
          labelText={(device) => device.backend}
          instant
          label="The ring of this cluster, with a new arc for {answered.name}"
        />
      </div>
    {/if}
    <p class="m-0 text-14 text-text-2">
      {answered.joined
        ? `${answered.name} joined the cluster. It appears on the ring as soon as its service starts.`
        : `${answered.name} was refused, and its ticket no longer works.`}
    </p>
  {:else if ticket}
    <p class="m-0 text-14 text-text-2">The new device scans this code — or, on a computer, run this command on it:</p>
    <div class="mx-auto"><QrCode svg={ticket.svg} label="The ticket of this invitation" /></div>
    <CodeBlock code={command} copyLabel="Copy the command" />
    <span class="text-12 text-text-3" role="timer">Valid for {countdown}. Then both screens show four symbols to compare.</span>
  {:else if failed}
    <Status kind="critical" label="This device could not open an invitation" />
    <p class="m-0 text-13 text-text-2">Only the device that created the cluster can add devices.</p>
  {:else}
    <Progress label="Opening an invitation" />
  {/if}
  {#snippet actions()}
    {#if attempt}
      <Button label="No, they're different" onclick={() => answer(false)} />
      <Button variant="primary" label="Yes, they match" onclick={() => answer(true)} />
    {:else if answered}
      <Button variant="primary" label="Done" onclick={() => (open = false)} />
    {/if}
  {/snippet}
</Dialog>
