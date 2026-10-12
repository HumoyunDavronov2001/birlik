<script lang="ts">
  import Icon from '../lib/components/Icon.svelte';
  import { ICON_NAMES } from '../lib/icons';
  import Gallery from './Gallery.svelte';
</script>

<Gallery columns={6}>
  {#each ICON_NAMES as name (name)}
    <span class="cell"><Icon {name} size={20} /><code>{name}</code></span>
  {/each}
</Gallery>

<style>
  .cell {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
  }
  code {
    font: 400 11px var(--mono);
    color: var(--text-3);
  }
</style>
