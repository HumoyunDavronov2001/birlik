import type { Meta, StoryObj } from '@storybook/svelte-vite';
import Switch from '../lib/components/Switch.svelte';

const meta = { title: 'Components/Switch', component: Switch, args: { label: 'Only while charging' } } satisfies Meta<typeof Switch>;
export default meta;
type Story = StoryObj<typeof meta>;

export const On: Story = { args: { checked: true } };
export const Off: Story = { args: { checked: false } };
export const Disabled: Story = { args: { checked: true, disabled: true } };
