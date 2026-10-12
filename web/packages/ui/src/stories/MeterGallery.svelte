<script lang="ts">
  import Meter from '../lib/components/Meter.svelte';
</script>

<div class="meters">
  <span>Accent</span><Meter value={0.72} label="Load" />
  {#each [1, 2, 3, 4, 5, 6, 7] as level (level)}
    <span>Level {level}</span><Meter value={level / 7} {level} label="Load level {level}" />
  {/each}
</div>

<style>
  .meters {
    display: grid;
    grid-template-columns: auto 240px;
    gap: 10px 16px;
    align-items: center;
    font-size: 12px;
    color: var(--text-2);
  }
</style>
