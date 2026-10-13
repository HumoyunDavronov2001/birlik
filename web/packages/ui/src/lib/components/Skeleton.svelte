<script lang="ts">
  // A placeholder while data is on its way: the shape of what comes, a light sweeping over it (still when motion is
  // reduced). Hidden from assistive technology; the screen says it is loading in words.
  interface Props {
    /** Lines of text. */
    lines?: number;
    /** A block instead of lines, in pixels. */
    height?: number;
  }

  let { lines = 3, height }: Props = $props();
</script>

<div class="om-skeleton" aria-hidden="true">
  {#if height}
    <span class="bar" style:height="{height}px"></span>
  {:else}
    {#each Array.from({ length: lines }, (_, i) => i) as i (i)}
      <span class="bar line" style:width={i === lines - 1 && lines > 1 ? '60%' : '100%'}></span>
    {/each}
  {/if}
</div>

<style>
  .om-skeleton {
    display: grid;
    gap: 8px;
  }
  .bar {
    position: relative;
    display: block;
    overflow: hidden;
    border-radius: 6px;
    background: var(--surface-2);
  }
  .line {
    height: 12px;
  }
  .bar::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, color-mix(in oklab, var(--text-1) 6%, transparent), transparent);
    transform: translateX(-100%);
    animation: om-sweep 1.6s var(--ease) infinite;
  }
  @keyframes om-sweep {
    to {
      transform: translateX(100%);
    }
  }
</style>
