import type { Meta, StoryObj } from '@storybook/svelte-vite';
import LivingRing from '../lib/components/LivingRing.svelte';

const meta = {
  title: 'Ring/Living',
  component: LivingRing,
  parameters: { layout: 'padded' },
  // WebGL output differs between GPUs and drivers, so this story is checked by axe but not by pixel snapshots.
  tags: ['no-visual'],
} satisfies Meta<typeof LivingRing>;
export default meta;

export const Default: StoryObj<typeof meta> = {
  args: {
    devices: [
      { name: 'Desktop', backend: 'CUDA', power: 3.0 },
      { name: 'MacBook', backend: 'Metal', power: 2.4 },
      { name: 'Phone', backend: 'Vulkan', power: 1.9 },
      { name: 'Laptop', backend: 'CPU', power: 1.6, slowdown: 1.25 },
      { name: 'Tablet', backend: 'WebGPU', power: 1.25 },
    ],
    reserve: [
      { name: 'iPhone', backend: 'Metal', power: 1.5 },
      { name: 'Server', backend: 'ROCm', power: 4.5 },
      { name: 'TV', backend: 'WebGPU', power: 0.9 },
    ],
  },
};
