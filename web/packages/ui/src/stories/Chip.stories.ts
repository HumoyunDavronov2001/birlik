import type { Meta, StoryObj } from '@storybook/svelte-vite';
import Chip from '../lib/components/Chip.svelte';

const meta = { title: 'Components/Chip', component: Chip, args: { label: 'Latency' } } satisfies Meta<typeof Chip>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Selected: Story = { args: { selected: true } };
export const WithStatusIcon: Story = { args: { label: '6 devices · all healthy', icon: 'ok', tone: 'good' } };
export const Mono: Story = { args: { label: 'Q4_K_M', mono: true } };
