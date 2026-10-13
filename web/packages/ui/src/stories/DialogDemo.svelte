<script lang="ts">
  import Button from '../lib/components/Button.svelte';
  import Dialog from '../lib/components/Dialog.svelte';
  import EmojiCheck from '../lib/components/EmojiCheck.svelte';

  const symbols = [
    { emoji: '🦁', name: 'Lion' },
    { emoji: '🍉', name: 'Watermelon' },
    { emoji: '🚀', name: 'Rocket' },
    { emoji: '🌙', name: 'Moon' },
  ];
</script>

<div style="max-width: 520px">
  <Dialog open inline title="Add a device" description="Pixel 9 Pro (Android) asks to join. Do these four symbols match the ones on its screen?">
    <EmojiCheck {symbols} />
    {#snippet actions()}
      <Button label="No, they're different" />
      <Button variant="primary" label="Yes, they match" />
    {/snippet}
  </Dialog>
</div>
