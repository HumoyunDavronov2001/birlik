<script lang="ts">
  interface Props {
    checked?: boolean;
    /** Accessible name; required because the switch has no visible text of its own. */
    label: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let { checked = $bindable(false), label, disabled = false, onchange }: Props = $props();

  function toggle() {
    if (disabled) return; // also guards against programmatic clicks
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button type="button" class="om-switch" role="switch" aria-checked={checked} aria-label={label} {disabled} onclick={toggle}
></button>

<style>
  .om-switch {
    position: relative;
    flex: none;
    width: 40px;
    height: 24px;
    padding: 0;
    border-radius: 999px;
    background: var(--surface-3);
    border: 1px solid var(--line-2);
    cursor: pointer;
  }
  .om-switch::after {
    content: '';
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text-2);
    transition:
      transform 0.2s var(--ease),
      background-color 0.2s var(--ease);
  }
  .om-switch[aria-checked='true'] {
    background: var(--turquoise);
    border-color: transparent;
  }
  .om-switch[aria-checked='true']::after {
    transform: translateX(16px);
    background: var(--bg);
  }
  .om-switch:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }
</style>
