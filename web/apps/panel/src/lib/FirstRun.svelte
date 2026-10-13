<script lang="ts">
  // The first run (PLAN.md §9.6, flow 1): this device belongs to no cluster yet. It creates one, with itself as the
  // admin, or joins one with the ticket another device shows. Once done, the service starts and the panel follows.
  import { ConnectError } from '@connectrpc/connect';
  import { Button, Card, Status } from '@omnarc/ui';
  import { api, cluster } from './cluster.svelte';
  import type { SasSymbol } from './gen/omnarc/v1/cluster_service_pb';

  let name = $state('Home cluster');
  let ticket = $state('');
  let deviceName = $state('');
  let busy = $state<'create' | 'join' | null>(null);
  let failed = $state('');
  let symbols = $state<SasSymbol[]>([]);
  let joined = $state('');

  const thisDevice = $derived(cluster.view.devices[cluster.view.thisNode]?.name ?? '');

  function message(e: unknown): string {
    return e instanceof ConnectError ? e.rawMessage : String(e);
  }

  async function create() {
    busy = 'create';
    failed = '';
    try {
      await api.createCluster({ name });
    } catch (e) {
      failed = message(e);
      busy = null;
    }
  }

  async function join() {
    busy = 'join';
    failed = '';
    symbols = [];
    try {
      for await (const response of api.joinCluster({ ticket, deviceName })) {
        if (response.step.case === 'symbols') symbols = response.step.value.symbols;
        if (response.step.case === 'joined') joined = response.step.value;
      }
    } catch (e) {
      failed = message(e);
      busy = null;
      symbols = [];
    }
  }
</script>

<div class="mx-auto grid w-full max-w-[880px] gap-5 py-4 md:py-12">
  <div class="grid gap-2">
    <h2 class="m-0 text-28 font-semibold">One cluster of your devices</h2>
    <p class="m-0 max-w-[620px] text-14 text-text-2">
      {thisDevice || 'This device'} belongs to no cluster yet. Create one and add your other devices to it, or join the
      cluster another device of yours already runs.
    </p>
  </div>

  {#if failed}<div role="alert"><Status kind="critical" label={failed} /></div>{/if}

  {#if joined}
    <Card title="Joined">
      <p class="m-0 text-14 text-text-2">This device joined {joined}. The ring of its devices opens in a moment.</p>
    </Card>
  {:else if symbols.length}
    <Card title="Compare the symbols">
      <p class="m-0 mb-4 text-14 text-text-2">
        The inviting device shows four symbols too. When its owner confirms they match, this device joins.
      </p>
      <ul class="m-0 grid list-none grid-cols-4 gap-2 p-0" aria-label="The symbols on this screen">
        {#each symbols as symbol, i (i)}
          <li class="grid justify-items-center gap-1 rounded-xl border border-line bg-surface-2 px-2 py-3">
            <span class="text-40 leading-none" aria-hidden="true">{symbol.emoji}</span>
            <span class="text-12 text-text-2">{symbol.name}</span>
          </li>
        {/each}
      </ul>
    </Card>
  {:else}
    <div class="grid gap-5 md:grid-cols-2">
      <Card title="Create a cluster">
        <form
          class="grid gap-3"
          onsubmit={(event) => {
            event.preventDefault();
            void create();
          }}
        >
          <label class="grid gap-1.5 text-13 text-text-2">
            Its name
            <input
              class="h-10 rounded-lg border border-line-2 bg-surface-2 px-3 text-14 text-text-1"
              bind:value={name}
              maxlength="64"
              required
            />
          </label>
          <p class="m-0 text-12 text-text-3">This device becomes its admin: it adds and removes the other devices.</p>
          <div><Button type="submit" variant="primary" label={busy === 'create' ? 'Creating…' : 'Create'} disabled={busy !== null} /></div>
        </form>
      </Card>

      <Card title="Join a cluster">
        <form
          class="grid gap-3"
          onsubmit={(event) => {
            event.preventDefault();
            void join();
          }}
        >
          <label class="grid gap-1.5 text-13 text-text-2">
            The ticket the other device shows
            <textarea
              class="min-h-24 rounded-lg border border-line-2 bg-surface-2 px-3 py-2 font-mono text-12 break-all text-text-1"
              bind:value={ticket}
              placeholder="OMNARC:…"
              required
            ></textarea>
          </label>
          <label class="grid gap-1.5 text-13 text-text-2">
            This device's name (optional)
            <input
              class="h-10 rounded-lg border border-line-2 bg-surface-2 px-3 text-14 text-text-1"
              bind:value={deviceName}
              placeholder={thisDevice}
              maxlength="64"
            />
          </label>
          <div><Button type="submit" label={busy === 'join' ? 'Joining…' : 'Join'} disabled={busy !== null} /></div>
        </form>
      </Card>
    </div>
  {/if}
</div>
