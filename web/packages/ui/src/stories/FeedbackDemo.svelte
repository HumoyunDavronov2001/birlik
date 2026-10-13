<script lang="ts">
  import Button from '../lib/components/Button.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Progress from '../lib/components/Progress.svelte';
  import Skeleton from '../lib/components/Skeleton.svelte';
  import Toaster from '../lib/components/Toaster.svelte';
  import Tooltip from '../lib/components/Tooltip.svelte';
  import Icon from '../lib/components/Icon.svelte';

  const sample = [
    { id: 1, kind: 'good' as const, title: 'Pixel 9 Pro joined', body: 'It took 6 layers.' },
    { id: 2, kind: 'info' as const, title: 'Laptop was removed', body: 'The cluster lets it in no more.' },
    { id: 3, kind: 'critical' as const, title: 'The relay does not answer', body: 'Devices on other networks cannot reach this one.' },
  ];
</script>

<div class="feedback">
  <section>
    <h3>Progress</h3>
    <Progress label="Downloading DeepSeek-R1-Distill-Llama-70B" value={18.4} max={42.5} detail="18.4 of 42.5 GB" />
    <Progress label="Fetching layers 40:80 from Desktop" value={0.73} />
    <Progress label="Measuring this device" />
  </section>
  <section>
    <h3>Notices</h3>
    <Toaster {sample} inline />
  </section>
  <section>
    <h3>Loading</h3>
    <Skeleton lines={3} />
    <Skeleton height={64} />
  </section>
  <section>
    <h3>Empty</h3>
    <EmptyState icon="cube" title="No models on this device yet" text="Download one once; the other devices fetch their layers from here.">
      {#snippet action()}<Button variant="primary" icon="plus" label="Add a model" />{/snippet}
    </EmptyState>
  </section>
  <section>
    <h3>Hint</h3>
    <div class="hint">
      <Tooltip text="Show the ring as a table" open inline>
        <span class="icon-button"><Icon name="table" label="Table" size={18} /></span>
      </Tooltip>
    </div>
  </section>
</div>

<style>
  .feedback {
    display: grid;
    gap: 28px;
    max-width: 520px;
  }
  section {
    display: grid;
    gap: 12px;
  }
  h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
  }
  .hint {
    display: flex;
    min-height: 72px;
    align-items: flex-start;
  }
  .icon-button {
    display: inline-flex;
    padding: 8px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    color: var(--text-2);
  }
</style>
