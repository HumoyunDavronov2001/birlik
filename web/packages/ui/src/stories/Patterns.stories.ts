import type { Meta, StoryObj } from '@storybook/svelte-vite';
import DialogDemo from './DialogDemo.svelte';
import FeedbackDemo from './FeedbackDemo.svelte';
import JoinPieces from './JoinPieces.svelte';
import TableDemo from './TableDemo.svelte';

// The components the stage B2 screens need (SPEC DS-05): dialogs, the pieces of joining, tables and feedback.
const meta = { title: 'Patterns' } satisfies Meta;
export default meta;
type Story = StoryObj;

export const Dialog: Story = { render: () => ({ Component: DialogDemo }) };
export const Joining: Story = { render: () => ({ Component: JoinPieces }) };
export const Table: Story = { render: () => ({ Component: TableDemo }) };
export const Feedback: Story = { render: () => ({ Component: FeedbackDemo }) };
