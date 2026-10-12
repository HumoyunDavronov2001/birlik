import type { Meta, StoryObj } from '@storybook/svelte-vite';
import Kbd from '../lib/components/Kbd.svelte';

const meta = { title: 'Components/Kbd', component: Kbd, args: { keys: '⌘K' } } satisfies Meta<typeof Kbd>;
export default meta;
export const Default: StoryObj<typeof meta> = {};
