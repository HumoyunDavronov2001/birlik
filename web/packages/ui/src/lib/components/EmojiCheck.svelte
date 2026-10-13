<script lang="ts">
  // The four symbols both screens show while a device joins (protocol/CLUSTER.md §2.4): big enough to compare at a
  // glance, each with its name for those who cannot see it.
  interface Symbol {
    emoji: string;
    name: string;
  }

  interface Props {
    symbols: Symbol[];
    /** What the list is, for assistive technology. */
    label?: string;
  }

  let { symbols, label = 'The symbols on this screen' }: Props = $props();
</script>

<ul class="om-emoji-check" aria-label={label}>
  {#each symbols as symbol, i (i)}
    <li>
      <span class="emoji" aria-hidden="true">{symbol.emoji}</span>
      <span class="name">{symbol.name}</span>
    </li>
  {/each}
</ul>

<style>
  .om-emoji-check {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 12px 8px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface-2);
  }
  .emoji {
    font-size: 40px;
    line-height: 1;
  }
  .name {
    font-size: 12px;
    color: var(--text-2);
    text-align: center;
  }
</style>
