import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Status from './Status.svelte';

describe('Status', () => {
  it('always shows a text label next to the decorative icon', () => {
    const { container } = render(Status, { kind: 'critical', label: 'Disconnected' });
    expect(screen.getByText('Disconnected')).toBeVisible();
    expect(container.querySelector('[aria-hidden="true"] svg, svg[aria-hidden="true"]')).not.toBeNull();
  });
});
