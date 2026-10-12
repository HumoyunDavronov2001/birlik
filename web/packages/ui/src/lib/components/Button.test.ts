import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Button from './Button.svelte';

describe('Button', () => {
  it('renders a button with its label', () => {
    render(Button, { label: 'Add a device', variant: 'primary' });
    const button = screen.getByRole('button', { name: 'Add a device' });
    expect(button).toHaveAttribute('type', 'button');
    expect(button).toHaveClass('primary');
  });

  it('renders a link when given an href', () => {
    render(Button, { label: 'Open models', href: '#models' });
    expect(screen.getByRole('link', { name: 'Open models' })).toHaveAttribute('href', '#models');
  });

  it('can be disabled', () => {
    render(Button, { label: 'Joining…', disabled: true });
    expect(screen.getByRole('button')).toBeDisabled();
  });
});
