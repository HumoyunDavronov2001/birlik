<script lang="ts">
  let { keys }: { keys: string } = $props();
</script>

<kbd class="om-kbd">{keys}</kbd>

<style>
  .om-kbd {
    font: 500 11px/1 var(--mono);
    padding: 3px 6px;
    border-radius: 6px;
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text-2);
  }
</style>
