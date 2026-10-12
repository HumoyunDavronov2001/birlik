import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import Switch from './Switch.svelte';

describe('Switch', () => {
  it('is a named switch that toggles on click and reports the new state', async () => {
    const onchange = vi.fn();
    render(Switch, { label: 'Only while charging', checked: false, onchange });
    const control = screen.getByRole('switch', { name: 'Only while charging' });
    expect(control).toHaveAttribute('aria-checked', 'false');
    await fireEvent.click(control);
    expect(control).toHaveAttribute('aria-checked', 'true');
    expect(onchange).toHaveBeenCalledWith(true);
  });

  it('does nothing while disabled', async () => {
    const onchange = vi.fn();
    render(Switch, { label: 'Only on Wi-Fi', checked: true, disabled: true, onchange });
    await fireEvent.click(screen.getByRole('switch'));
    expect(onchange).not.toHaveBeenCalled();
  });
});
