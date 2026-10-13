<script lang="ts">
  // What a screen shows before it has anything: what will be here and the one action that fills it.
  import type { Snippet } from 'svelte';
  import type { IconName } from '../icons';
  import Icon from './Icon.svelte';

  interface Props {
    icon: IconName;
    title: string;
    text?: string;
    action?: Snippet;
  }

  let { icon, title, text, action }: Props = $props();
</script>

<div class="om-empty">
  <span class="icon"><Icon name={icon} size={28} /></span>
  <p class="title">{title}</p>
  {#if text}<p class="text">{text}</p>{/if}
  {#if action}<div class="action">{@render action()}</div>{/if}
</div>

<style>
  .om-empty {
    display: grid;
    justify-items: center;
    gap: 8px;
    padding: 40px 24px;
    border: 1px dashed var(--line-2);
    border-radius: 16px;
    text-align: center;
  }
  .icon {
    display: inline-flex;
    color: var(--text-3);
  }
  p {
    margin: 0;
    max-width: 420px;
  }
  .title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-1);
  }
  .text {
    font-size: 13px;
    color: var(--text-2);
  }
  .action {
    margin-top: 8px;
  }
</style>
