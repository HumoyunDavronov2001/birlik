import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Meter from './Meter.svelte';

describe('Meter', () => {
  it('exposes its value as a percentage and clamps it', () => {
    render(Meter, { value: 0.674, label: 'Memory' });
    expect(screen.getByRole('meter', { name: 'Memory' })).toHaveAttribute('aria-valuenow', '67');
  });

  it('never goes above 100', () => {
    render(Meter, { value: 3, label: 'Load', level: 7 });
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '100');
  });
});
