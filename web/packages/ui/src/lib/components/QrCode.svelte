<script lang="ts">
  // A QR code on a white plate, so any camera reads it in either theme. The SVG comes from this device's own service.
  interface Props {
    /** The QR code's SVG markup. */
    svg: string;
    /** What it holds, for assistive technology. */
    label: string;
    size?: number;
  }

  let { svg, label, size = 240 }: Props = $props();
</script>

<div class="om-qr" role="img" aria-label={label} style:width="{size}px">
  <!-- eslint-disable-next-line svelte/no-at-html-tags -- the SVG comes from this device's own service -->
  {@html svg}
</div>

<style>
  .om-qr {
    max-width: 100%;
    padding: 8px;
    border-radius: 10px;
    background: #ffffff;
  }
  .om-qr :global(svg) {
    display: block;
    width: 100%;
    height: auto;
  }
</style>
