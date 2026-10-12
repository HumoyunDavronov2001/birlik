<script lang="ts">
  import { Card, Chip } from '@omnarc/ui';
  import { cluster } from '$lib/cluster.svelte';
  import { deviceName } from '$lib/model';
  import { currentTheme, setTheme, type Theme } from '$lib/theme';

  let theme = $state<Theme>(currentTheme());
  const themes: { value: Theme; label: string }[] = [
    { value: 'system', label: 'Like the system' },
    { value: 'dark', label: 'Dark' },
    { value: 'light', label: 'Light' },
  ];

  function pick(value: Theme) {
    theme = value;
    setTheme(value);
  }
</script>

<svelte:head><title>Settings · {cluster.view.name || 'Omnarc'}</title></svelte:head>

<div class="grid max-w-[720px] gap-5">
  <Card title="Theme">
    <div class="flex flex-wrap gap-2" role="radiogroup" aria-label="Theme">
      {#each themes as option (option.value)}
        <button
          class="cursor-pointer border-0 bg-transparent p-0"
          role="radio"
          aria-checked={theme === option.value}
          onclick={() => pick(option.value)}
        >
          <Chip label={option.label} selected={theme === option.value} />
        </button>
      {/each}
    </div>
  </Card>

  <Card title="This device">
    <dl class="m-0 grid gap-3 text-13">
      <div class="grid gap-1 sm:grid-cols-[160px_1fr]">
        <dt class="text-text-3">Name</dt>
        <dd class="m-0">{cluster.view.thisNode ? deviceName(cluster.view, cluster.view.thisNode) : '—'}</dd>
      </div>
      <div class="grid gap-1 sm:grid-cols-[160px_1fr]">
        <dt class="text-text-3">Node id</dt>
        <dd class="m-0 font-mono text-12 break-all text-text-2">{cluster.view.thisNode || '—'}</dd>
      </div>
      <div class="grid gap-1 sm:grid-cols-[160px_1fr]">
        <dt class="text-text-3">Cluster</dt>
        <dd class="m-0">{cluster.view.name || '—'}</dd>
      </div>
      <div class="grid gap-1 sm:grid-cols-[160px_1fr]">
        <dt class="text-text-3">Cluster id</dt>
        <dd class="m-0 font-mono text-12 break-all text-text-2">{cluster.view.id || '—'}</dd>
      </div>
    </dl>
    <p class="m-0 mt-4 text-12 text-text-3">
      Nothing is sent anywhere by default. This panel talks only to this device's service, and only this device can open it.
    </p>
  </Card>
</div>
