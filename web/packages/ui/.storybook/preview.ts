import type { Preview } from '@storybook/svelte-vite';
import '../src/lib/styles/base.css';

const preview: Preview = {
  globalTypes: {
    theme: {
      description: 'Color theme',
      toolbar: {
        title: 'Theme',
        icon: 'contrast',
        items: [
          { value: 'dark', title: 'Dark' },
          { value: 'light', title: 'Light' },
        ],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: { theme: 'dark' },
  decorators: [
    (story, context) => {
      document.documentElement.dataset.theme = String(context.globals.theme ?? 'dark');
      return story();
    },
  ],
  parameters: { layout: 'centered' },
};

export default preview;
