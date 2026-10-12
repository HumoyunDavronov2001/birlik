<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { IconName } from '../icons';
  import Icon from './Icon.svelte';

  interface Props {
    label?: string;
    icon?: IconName;
    /** Colors the icon with a status color (the label stays neutral). */
    tone?: 'good' | 'warning' | 'serious' | 'critical';
    selected?: boolean;
    mono?: boolean;
    children?: Snippet;
  }

  let { label, icon, tone, selected = false, mono = false, children }: Props = $props();
</script>

<span class="om-chip" class:selected class:mono>
  {#if icon}<span class="icon" data-tone={tone}><Icon name={icon} size={14} /></span>{/if}
  {#if children}{@render children()}{:else}{label}{/if}
</span>

<style>
  .om-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 10px;
    border-radius: 999px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
  }
  .selected {
    color: var(--text-1);
    border-color: color-mix(in oklab, var(--turquoise) 55%, transparent);
    background: color-mix(in oklab, var(--turquoise) 12%, var(--surface-2));
  }
  .mono {
    font-family: var(--mono);
  }
  .icon {
    display: inline-flex;
  }
  .icon[data-tone='good'] {
    color: var(--status-good);
  }
  .icon[data-tone='warning'] {
    color: var(--status-warning);
  }
  .icon[data-tone='serious'] {
    color: var(--status-serious);
  }
  .icon[data-tone='critical'] {
    color: var(--status-critical);
  }
</style>
