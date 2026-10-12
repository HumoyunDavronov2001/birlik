<script lang="ts">
  import Logo from '../lib/components/Logo.svelte';
  import Gallery from './Gallery.svelte';
</script>

<Gallery columns={5}>
  <Logo size={96} title="Omnarc" />
  <Logo size={48} />
  <span class="label">Tray</span>
  <Logo size={26} load={0.12} title="Load 12%" />
  <Logo size={26} load={0.92} title="Load 92%" />
</Gallery>

<style>
  .label {
    font-size: 12px;
    color: var(--text-3);
  }
</style>
