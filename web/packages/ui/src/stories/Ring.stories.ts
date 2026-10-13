import type { Meta, StoryObj } from '@storybook/svelte-vite';
import Ring from '../lib/components/Ring.svelte';

import type { RingDeviceView } from '../lib/components/Ring.svelte';

// the phone charges, the laptop runs hot (and is the slowest stage), the tablet's battery runs low
const HOME: RingDeviceView[] = [
  { name: 'Desktop PC', layers: 24, level: 6, ms: 9, draftMs: 3 },
  { name: 'Pixel 9 Pro', layers: 6, level: 6, ms: 12, markers: [{ icon: 'bolt', kind: 'good', label: 'Charging · 81%' }] },
  { name: 'Laptop', layers: 8, level: 7, ms: 14, markers: [{ icon: 'temp', kind: 'serious', label: 'Hot' }] },
  { name: 'iPhone 16 Pro', layers: 4, level: 4, ms: 8 },
  {
    name: 'Tablet',
    layers: 4,
    level: 5,
    ms: 9,
    markers: [
      { icon: 'battery', kind: 'warning', label: 'On battery · 18%' },
      { icon: 'temp', kind: 'warning', label: 'Warm' },
    ],
  },
  { name: 'MacBook Pro', layers: 18, level: 6, ms: 11 },
];

const meta = { title: 'Ring', component: Ring, parameters: { layout: 'padded' } } satisfies Meta<typeof Ring>;
export default meta;
type Story = StoryObj<typeof meta>;

export const HomeCluster: Story = {
  args: {
    devices: HOME,
    hops: [0.4, 3.8, 4.4, 3.6, 3.3, 0.6],
    tokens: 5,
    center: { value: '18.4', unit: 'tokens/s', caption: 'Qwen3-32B · Q4_K_M' },
    label: 'Cluster ring: 6 devices; the slowest stage is the Laptop at 14 ms',
  },
};

export const Contribute: Story = {
  args: {
    devices: HOME,
    variant: 'compact',
    highlight: 1,
    tokens: 3,
    center: { value: '+6', unit: 'layers' },
    label: 'Your own arc is highlighted on the ring',
  },
};
