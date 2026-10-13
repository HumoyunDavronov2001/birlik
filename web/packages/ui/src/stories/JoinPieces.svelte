<script lang="ts">
  import CodeBlock from '../lib/components/CodeBlock.svelte';
  import QrCode from '../lib/components/QrCode.svelte';
  import qr from './sample-qr.svg?raw';
</script>

<div class="pieces">
  <QrCode svg={qr} label="A sample cluster ticket" size={200} />
  <CodeBlock code="omnarc-node join OMNARC:BIRAUIGJRXB7RQ4N2KPZ6WD5MXHCLYE3TVFS7GAQOJ" copyLabel="Copy the command" />
</div>

<style>
  .pieces {
    display: grid;
    gap: 16px;
    max-width: 420px;
  }
</style>
