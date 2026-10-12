<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title?: string;
    padding?: 'md' | 'lg';
    children?: Snippet;
  }

  let { title, padding = 'md', children }: Props = $props();
</script>

<section class="bk-card {padding}">
  {#if title}<h3>{title}</h3>{/if}
  {@render children?.()}
</section>

<style>
  .bk-card {
    display: grid;
    gap: 12px;
    align-content: start;
    background: var(--surface-1);
    border: 1px solid var(--line);
    border-radius: 16px;
  }
  .md {
    padding: 16px;
  }
  .lg {
    padding: 24px;
  }
  h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
</style>
