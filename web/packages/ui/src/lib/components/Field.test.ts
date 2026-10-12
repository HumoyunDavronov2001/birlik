import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Field from './Field.svelte';

describe('Field', () => {
  it('labels its input', () => {
    render(Field, { label: 'Cluster name', value: 'Home cluster' });
    expect(screen.getByLabelText('Cluster name')).toHaveValue('Home cluster');
  });

  it('describes an error and marks the input invalid', () => {
    render(Field, { label: 'Join code', value: '4827 19', error: 'The code has 8 digits' });
    const input = screen.getByLabelText('Join code');
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input).toHaveAccessibleDescription('The code has 8 digits');
  });
});
