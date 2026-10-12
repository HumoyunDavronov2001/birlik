<script lang="ts">
  import type { Snippet } from 'svelte';

  let { children, columns = 4 }: { children: Snippet; columns?: number } = $props();
</script>

<div class="gallery" style:--columns={columns}>{@render children()}</div>

<style>
  .gallery {
    display: grid;
    grid-template-columns: repeat(var(--columns), auto);
    gap: 16px 20px;
    align-items: center;
    justify-items: start;
  }
</style>
