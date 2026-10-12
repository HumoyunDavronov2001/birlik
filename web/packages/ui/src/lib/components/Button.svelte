<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { IconName } from '../icons';
  import Icon from './Icon.svelte';

  interface Props extends HTMLButtonAttributes {
    variant?: 'primary' | 'secondary' | 'ghost';
    size?: 'md' | 'sm';
    icon?: IconName;
    label?: string;
    /** Renders a link that looks like a button. */
    href?: string;
    children?: Snippet;
  }

  let { variant = 'secondary', size = 'md', icon, label, href, children, type = 'button', ...rest }: Props = $props();
</script>

{#snippet content()}
  {#if icon}<Icon name={icon} size={size === 'sm' ? 14 : 16} />{/if}
  {#if children}{@render children()}{:else}{label}{/if}
{/snippet}

{#if href}
  <a class="bk-btn {variant} {size}" {href}>{@render content()}</a>
{:else}
  <button class="bk-btn {variant} {size}" {type} {...rest}>{@render content()}</button>
{/if}

<style>
  .bk-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 40px;
    padding: 0 16px;
    border-radius: 10px;
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text-1);
    font: 500 14px/1 var(--sans);
    text-decoration: none;
    cursor: pointer;
    transition:
      transform 0.12s var(--ease),
      background-color 0.2s var(--ease);
  }
  .bk-btn:hover {
    background: var(--surface-3);
  }
  .bk-btn:active {
    transform: scale(0.97);
  }
  .bk-btn:disabled {
    cursor: not-allowed;
    opacity: 0.5;
    transform: none;
  }
  .primary {
    background: var(--turquoise);
    border-color: transparent;
    color: var(--bg);
  }
  .primary:hover {
    background: var(--turquoise-strong);
  }
  .ghost {
    background: transparent;
  }
  .sm {
    height: 32px;
    padding: 0 12px;
    font-size: 13px;
    border-radius: 8px;
  }
</style>
