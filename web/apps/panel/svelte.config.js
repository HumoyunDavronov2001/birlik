import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** The panel is a single-page app: the node serves build/ and falls back to index.html for every route. */
export default {
  preprocess: vitePreprocess(),
  kit: { adapter: adapter({ fallback: 'index.html' }) },
};
