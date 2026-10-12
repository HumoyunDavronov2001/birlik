<script lang="ts">
  import type { IconName } from '../icons';
  import Chip from './Chip.svelte';

  type Kind = 'good' | 'warning' | 'serious' | 'critical';
  interface Props {
    kind: Kind;
    /** Always required: a status is never shown by color alone. */
    label: string;
  }

  let { kind, label }: Props = $props();
  const ICON: Record<Kind, IconName> = { good: 'ok', warning: 'warn', serious: 'temp', critical: 'error' };
</script>

<Chip icon={ICON[kind]} tone={kind} {label} />
