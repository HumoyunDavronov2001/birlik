import { readFileSync } from 'node:fs';
import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';

interface Entry {
  id: string;
  type: string;
  tags?: string[];
}

const index = JSON.parse(readFileSync(new URL('../../storybook-static/index.json', import.meta.url), 'utf8')) as {
  entries: Record<string, Entry>;
};
const stories = Object.values(index.entries).filter((entry) => entry.type === 'story');

for (const theme of ['dark', 'light'] as const) {
  for (const story of stories) {
    test(`${story.id} · ${theme}`, async ({ page }) => {
      const errors: string[] = [];
      page.on('pageerror', (error) => errors.push(error.message));
      await page.goto(`/iframe.html?id=${story.id}&viewMode=story&globals=theme:${theme}`);
      const root = page.locator('#storybook-root');
      await expect(root.locator(':scope > *').first()).toBeAttached(); // rendered (a switch has no text)
      await page.evaluate(() => document.fonts.ready);

      const axe = await new AxeBuilder({ page })
        .include('#storybook-root')
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
        .analyze();
      expect(axe.violations.map((v) => `${v.id}: ${v.help} (${v.nodes.length})`)).toEqual([]);

      if (!story.tags?.includes('no-visual')) {
        await expect(root).toHaveScreenshot(`${story.id}-${theme}.png`);
      }
      expect(errors).toEqual([]);
    });
  }
}
