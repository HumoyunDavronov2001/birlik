<script lang="ts">
  import Card from '../lib/components/Card.svelte';
  import Meter from '../lib/components/Meter.svelte';
  import Status from '../lib/components/Status.svelte';
</script>

<div style="width: 320px">
  <Card title="Desktop PC">
    <p class="note">RTX 4070 · CUDA · 24 layers</p>
    <Meter value={0.83} level={6} label="Busy" />
    <Status kind="good" label="Healthy" />
  </Card>
</div>

<style>
  .note {
    margin: 0;
    font-size: 13px;
    color: var(--text-2);
  }
</style>
