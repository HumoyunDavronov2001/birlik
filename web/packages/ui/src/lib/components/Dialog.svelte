<script lang="ts">
  // A modal dialog (DS-05): a title, a description, the content and the actions. Escape and the backdrop close it, and
  // focus stays inside while it is open (bits-ui). `inline` draws it in place, for documentation and tests.
  import type { Snippet } from 'svelte';
  import { Dialog } from 'bits-ui';
  import Icon from './Icon.svelte';

  interface Props {
    open?: boolean;
    title: string;
    description?: string;
    /** 'sm' 440 px wide, 'md' 520 px. */
    size?: 'sm' | 'md';
    /** A close button in the corner. */
    closable?: boolean;
    /** Drawn in place instead of over the page. */
    inline?: boolean;
    children?: Snippet;
    actions?: Snippet;
  }

  let {
    open = $bindable(false),
    title,
    description,
    size = 'md',
    closable = true,
    inline = false,
    children,
    actions,
  }: Props = $props();
</script>

<Dialog.Root bind:open>
  <Dialog.Portal disabled={inline}>
    {#if !inline}<Dialog.Overlay class="om-dialog-overlay" />{/if}
    <Dialog.Content
      class="om-dialog {size} {inline ? 'inline' : ''}"
      trapFocus={!inline}
      preventScroll={!inline}
      onOpenAutoFocus={(event) => inline && event.preventDefault()}
    >
      <div class="om-dialog-head">
        <Dialog.Title class="om-dialog-title">{title}</Dialog.Title>
        {#if closable}
          <Dialog.Close class="om-dialog-close" aria-label="Close"><Icon name="close" size={16} /></Dialog.Close>
        {/if}
      </div>
      {#if description}<Dialog.Description class="om-dialog-description">{description}</Dialog.Description>{/if}
      {#if children}<div class="om-dialog-body">{@render children()}</div>{/if}
      {#if actions}<div class="om-dialog-actions">{@render actions()}</div>{/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  /* bits-ui renders these elements, and a portal moves them to the end of the page: the styles are global, named om- */
  :global(.om-dialog-overlay) {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.6);
    animation: om-fade 200ms var(--ease) backwards;
  }
  :global(.om-dialog) {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 50;
    display: grid;
    gap: 16px;
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100dvh - 32px);
    overflow: auto;
    padding: 24px;
    border: 1px solid var(--line-2);
    border-radius: 16px;
    background: var(--surface-1);
    color: var(--text-1);
    box-shadow: 0 24px 64px rgb(0 0 0 / 0.35);
    transform: translate(-50%, -50%);
    animation: om-rise 320ms var(--ease) backwards;
  }
  :global(.om-dialog.sm) {
    width: min(440px, calc(100vw - 32px));
  }
  :global(.om-dialog.inline) {
    position: relative;
    top: auto;
    left: auto;
    z-index: auto;
    transform: none;
    animation: none;
    box-shadow: none;
  }
  :global(.om-dialog-head) {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  :global(.om-dialog-title) {
    flex: 1;
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  :global(.om-dialog-close) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    margin: -4px -4px 0 0;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
  }
  :global(.om-dialog-close:hover) {
    background: var(--surface-2);
    color: var(--text-1);
  }
  :global(.om-dialog-description) {
    margin: 0;
    font-size: 14px;
    color: var(--text-2);
  }
  :global(.om-dialog-body) {
    display: grid;
    gap: 12px;
  }
  :global(.om-dialog-actions) {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }
  @keyframes -global-om-fade {
    from {
      opacity: 0;
    }
  }
  @keyframes -global-om-rise {
    from {
      opacity: 0;
      transform: translate(-50%, calc(-50% + 8px));
    }
  }
</style>
