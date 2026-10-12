import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  // in development the node's API answers behind the dev server: omnarc-node serve
  server: { proxy: { '/omnarc.v1.ClusterService': 'http://127.0.0.1:7600' } },
});
