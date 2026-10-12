import type { Meta, StoryObj } from '@storybook/svelte-vite';
import CardDemo from './CardDemo.svelte';
import IconGallery from './IconGallery.svelte';
import LogoGallery from './LogoGallery.svelte';
import MeterGallery from './MeterGallery.svelte';
import StatusGallery from './StatusGallery.svelte';

const meta = { title: 'Foundations' } satisfies Meta;
export default meta;
type Story = StoryObj;

export const Status: Story = { render: () => ({ Component: StatusGallery }) };
export const Meters: Story = { render: () => ({ Component: MeterGallery }) };
export const Card: Story = { render: () => ({ Component: CardDemo }) };
export const Icons: Story = { render: () => ({ Component: IconGallery }) };
export const Logo: Story = { render: () => ({ Component: LogoGallery }) };
