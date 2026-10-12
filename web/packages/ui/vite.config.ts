import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig } from 'vitest/config';

// The design tokens and the living ring live in /design at the repository root.
const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));

export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  server: { fs: { allow: [repoRoot] } },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    setupFiles: ['./vitest.setup.ts'],
  },
});
