<script lang="ts">
  import { ICONS, type IconName } from '../icons';

  interface Props {
    name: IconName;
    size?: number;
    /** Accessible name; without it the icon is decorative and hidden from assistive technology. */
    label?: string;
  }

  let { name, size = 18, label }: Props = $props();
  const markup = $derived(
    `<svg viewBox="0 0 24 24" width="${size}" height="${size}" aria-hidden="true" focusable="false">${ICONS[name]}</svg>`,
  );
</script>

<span
  class="bk-icon"
  style:width="{size}px"
  style:height="{size}px"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
>
  {@html markup}
</span>

<style>
  .bk-icon {
    display: inline-flex;
    flex: none;
  }
  .bk-icon :global(svg) {
    display: block;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.75;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
