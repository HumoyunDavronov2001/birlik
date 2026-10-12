<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';

  interface Props extends HTMLInputAttributes {
    label: string;
    value?: string;
    hint?: string;
    error?: string;
  }

  let { label, value = $bindable(''), hint, error, ...rest }: Props = $props();
  const id = $props.id();
</script>

<div class="bk-field">
  <label for={id}>{label}</label>
  <input
    {id}
    bind:value
    aria-invalid={error ? 'true' : undefined}
    aria-describedby={error || hint ? `${id}-note` : undefined}
    {...rest}
  />
  {#if error}
    <span class="note error" id="{id}-note">{error}</span>
  {:else if hint}
    <span class="note" id="{id}-note">{hint}</span>
  {/if}
</div>

<style>
  .bk-field {
    display: grid;
    gap: 6px;
    width: 100%;
    max-width: 320px;
  }
  label {
    font-size: 12px;
    color: var(--text-2);
  }
  input {
    height: 40px;
    border-radius: 10px;
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text-1);
    padding: 0 12px;
    font: 400 14px var(--sans);
  }
  input:focus {
    outline: 2px solid var(--turquoise);
    outline-offset: 1px;
  }
  input[aria-invalid='true'] {
    border-color: var(--status-critical);
  }
  .note {
    font-size: 12px;
    color: var(--text-3);
  }
  .error {
    color: var(--text-1);
  }
  .error::before {
    content: '● ';
    color: var(--status-critical);
  }
</style>
