<script lang="ts">
  // Where notices appear: bottom right on a large screen, along the bottom on a phone. `inline` lays them out in place,
  // for documentation and tests.
  import type { IconName } from '../icons';
  import { toasts, type Toast, type ToastKind } from '../toasts.svelte';
  import Icon from './Icon.svelte';

  interface Props {
    /** Notices to show instead of the live ones (documentation and tests). */
    sample?: Toast[];
    inline?: boolean;
  }

  let { sample, inline = false }: Props = $props();
  const shown = $derived(sample ?? toasts.list);
  const ICON: Record<ToastKind, IconName> = { good: 'ok', info: 'info', warning: 'warn', critical: 'error' };
</script>

<div class="om-toaster" class:inline role="region" aria-label="Notices">
  {#each shown as toast (toast.id)}
    <div class="toast" role={toast.kind === 'critical' ? 'alert' : 'status'}>
      <span class="icon" data-kind={toast.kind}><Icon name={ICON[toast.kind]} size={18} /></span>
      <div class="text">
        <p class="title">{toast.title}</p>
        {#if toast.body}<p class="body">{toast.body}</p>{/if}
      </div>
      <button type="button" aria-label="Close" onclick={() => toasts.dismiss(toast.id)}>
        <Icon name="close" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .om-toaster {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 60;
    display: grid;
    gap: 8px;
    width: min(380px, calc(100vw - 32px));
    pointer-events: none;
  }
  .om-toaster.inline {
    position: static;
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: var(--surface-1);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.25);
    pointer-events: auto;
    animation: om-toast 200ms var(--ease) backwards;
  }
  .icon {
    display: inline-flex;
    padding-top: 1px;
  }
  .icon[data-kind='good'] {
    color: var(--status-good);
  }
  .icon[data-kind='info'] {
    color: var(--turquoise);
  }
  .icon[data-kind='warning'] {
    color: var(--status-warning);
  }
  .icon[data-kind='critical'] {
    color: var(--status-critical);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  p {
    margin: 0;
  }
  .title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
  }
  .body {
    margin-top: 2px;
    font-size: 12px;
    color: var(--text-2);
  }
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
  }
  button:hover {
    background: var(--surface-2);
    color: var(--text-1);
  }
  @keyframes om-toast {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
