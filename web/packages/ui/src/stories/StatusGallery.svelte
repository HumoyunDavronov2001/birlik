<script lang="ts">
  import Status from '../lib/components/Status.svelte';
  import Gallery from './Gallery.svelte';
</script>

<Gallery>
  <Status kind="good" label="Healthy" />
  <Status kind="warning" label="Warning" />
  <Status kind="serious" label="Hot" />
  <Status kind="critical" label="Disconnected" />
</Gallery>
