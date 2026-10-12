import { defineConfig, devices } from '@playwright/test';

// Every story is checked in both themes: axe (WCAG 2.2 AA) and a pixel snapshot.
export default defineConfig({
  testDir: 'tests/visual',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: { baseURL: 'http://127.0.0.1:6007', reducedMotion: 'reduce' },
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.01, animations: 'disabled' } },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1024, height: 768 } } }],
  webServer: {
    command: 'node scripts/serve.mjs storybook-static 6007',
    url: 'http://127.0.0.1:6007/index.json',
    reuseExistingServer: !process.env.CI,
  },
});
