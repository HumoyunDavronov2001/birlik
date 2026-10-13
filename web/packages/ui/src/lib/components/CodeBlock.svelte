<script lang="ts">
  // A command or a code to copy: monospace, wrapped anywhere, with a copy button that says when it copied.
  import Icon from './Icon.svelte';

  interface Props {
    code: string;
    /** The copy button's name for assistive technology. */
    copyLabel?: string;
    /** Lines shown before it scrolls. */
    lines?: number;
  }

  let { code, copyLabel = 'Copy', lines = 4 }: Props = $props();
  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy() {
    await navigator.clipboard.writeText(code);
    copied = true;
    clearTimeout(timer);
    timer = setTimeout(() => (copied = false), 2000);
  }
</script>

<div class="om-code">
  <code style:max-height="{lines * 1.6 + 1}em">{code}</code>
  <button type="button" onclick={copy} aria-label={copied ? 'Copied' : copyLabel}>
    <Icon name={copied ? 'ok' : 'copy'} size={16} />
  </button>
  <span class="om-visually-hidden" aria-live="polite">{copied ? 'Copied' : ''}</span>
</div>

<style>
  .om-code {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 8px 8px 12px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface-2);
  }
  code {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 4px 0;
    font: 400 12px/1.6 var(--mono);
    color: var(--text-2);
    word-break: break-all;
    white-space: pre-wrap;
  }
  button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: var(--surface-1);
    color: var(--text-2);
    cursor: pointer;
  }
  button:hover {
    color: var(--text-1);
  }
</style>
