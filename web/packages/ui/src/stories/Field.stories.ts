import type { Meta, StoryObj } from '@storybook/svelte-vite';
import Field from '../lib/components/Field.svelte';

const meta = { title: 'Components/Field', component: Field, args: { label: 'Cluster name', value: 'Home cluster' } } satisfies Meta<typeof Field>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const WithHint: Story = { args: { hint: 'Shown on every device in the cluster' } };
export const WithError: Story = { args: { label: 'Join code', value: '4827 19', error: 'The code has 8 digits' } };
