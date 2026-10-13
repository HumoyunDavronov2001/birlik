<script lang="ts">
  // A short hint on hover or focus, for a control that shows only an icon. `open` and `inline` show it in place, for
  // documentation and tests.
  import type { Snippet } from 'svelte';
  import { Tooltip } from 'bits-ui';

  interface Props {
    text: string;
    open?: boolean;
    inline?: boolean;
    /** The control the hint belongs to: the trigger is that button. */
    onclick?: () => void;
    /** The button's name for assistive technology. */
    label?: string;
    /** A toggle's state. */
    pressed?: boolean;
    /** Classes of the trigger button. */
    class?: string;
    children: Snippet;
  }

  let {
    text,
    open = $bindable(false),
    inline = false,
    onclick,
    label,
    pressed,
    class: className = '',
    children,
  }: Props = $props();
</script>

{#if inline}
  <!-- documentation and tests: the hint drawn under its control, without floating -->
  <span class="om-tooltip-inline">
    {@render children()}
    <span class="om-tooltip" role="tooltip">{text}</span>
  </span>
{:else}
  <Tooltip.Provider delayDuration={300}>
    <Tooltip.Root bind:open>
      <Tooltip.Trigger class="om-tooltip-trigger {className}" aria-label={label} aria-pressed={pressed} {onclick}>
        {@render children()}
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content class="om-tooltip" sideOffset={6}>{text}</Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  </Tooltip.Provider>
{/if}

<style>
  /* the control a hint explains is an icon button: 36 px, a hairline, the icon in a text color */
  :global(.om-tooltip-trigger) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    padding: 0;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: transparent;
    color: var(--text-2);
    font: inherit;
    cursor: pointer;
  }
  :global(.om-tooltip-trigger:hover),
  :global(.om-tooltip-trigger[aria-pressed='true']) {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .om-tooltip-inline {
    display: inline-grid;
    justify-items: start;
    gap: 6px;
  }
  :global(.om-tooltip) {
    z-index: 70;
    max-width: 260px;
    padding: 6px 10px;
    border: 1px solid var(--line-2);
    border-radius: 6px;
    background: var(--surface-3);
    font-size: 12px;
    color: var(--text-1);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
  }
</style>
