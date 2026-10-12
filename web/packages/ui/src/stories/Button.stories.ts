import type { Meta, StoryObj } from '@storybook/svelte-vite';
import Button from '../lib/components/Button.svelte';

const meta = { title: 'Components/Button', component: Button, args: { label: 'Add a device' } } satisfies Meta<typeof Button>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Primary: Story = { args: { variant: 'primary', icon: 'plus' } };
export const Secondary: Story = { args: { variant: 'secondary' } };
export const Ghost: Story = { args: { variant: 'ghost', label: 'Dismiss' } };
export const Small: Story = { args: { variant: 'primary', size: 'sm', icon: 'plus', label: 'Add device' } };
export const Disabled: Story = { args: { variant: 'primary', disabled: true, label: 'Joining…' } };
export const AsLink: Story = { args: { href: '#models', label: 'Open models' } };
