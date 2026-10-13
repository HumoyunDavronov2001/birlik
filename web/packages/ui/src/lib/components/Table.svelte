<script module lang="ts">
  // the rows' type defaults to any: a list of columns is often written apart from its table
  export interface Column<Row = any> {
    key: string;
    label: string;
    /** Numbers line up at the end. */
    align?: 'start' | 'end';
    mono?: boolean;
    /** The header is read out but not shown, as for a column of actions. */
    hideLabel?: boolean;
    /** May take more than one line on a narrow screen, as a name; otherwise a row is one line. */
    wrap?: boolean;
    /** The value the rows sort by when the header is pressed; pressed again, they reverse. */
    sort?(row: Row): string | number;
  }
</script>

<script lang="ts" generics="Row">
  // A data table (DS-05): a caption for assistive technology, headers that say what they head, numbers aligned to the
  // end in monospace, and a line for an empty table. A cell snippet draws what is not plain text. A column with a sort
  // value has a header button; the sorted header says its order to assistive technology (aria-sort).
  import type { Snippet } from 'svelte';

  interface Props {
    caption: string;
    /** The caption shows above the table; otherwise only assistive technology reads it. */
    captionVisible?: boolean;
    columns: Column<Row>[];
    rows: Row[];
    key: (row: Row) => string;
    cell?: Snippet<[Row, Column]>;
    /** What an empty table says. */
    empty?: string;
    /** The column the rows are sorted by, if any; pressing a sortable header changes it. */
    order?: { key: string; ascending: boolean } | null;
  }

  let {
    caption,
    captionVisible = false,
    columns,
    rows,
    key,
    cell,
    empty = 'Nothing here yet',
    order = $bindable(null),
  }: Props = $props();

  function text(row: Row, column: Column<Row>): string {
    const value = (row as Record<string, unknown>)[column.key];
    return value === undefined || value === null ? '—' : String(value);
  }

  const shown = $derived.by(() => {
    const by = order && columns.find((column) => column.key === order?.key)?.sort;
    if (!order || !by) return rows;
    return [...rows].sort((a, b) => {
      const x = by(a);
      const y = by(b);
      const compared =
        typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y), undefined, { numeric: true });
      return order?.ascending === false ? -compared : compared;
    });
  });

  function sortBy(column: Column<Row>) {
    order = { key: column.key, ascending: order?.key === column.key ? !order.ascending : true };
  }
</script>

<div class="om-table">
  <table>
    <caption class:om-visually-hidden={!captionVisible}>{caption}</caption>
    <thead>
      <tr>
        {#each columns as column (column.key)}
          {@const sorted = order?.key === column.key ? (order.ascending ? 'ascending' : 'descending') : undefined}
          <th scope="col" class:end={column.align === 'end'} aria-sort={sorted}>
            {#if column.hideLabel}
              <span class="om-visually-hidden">{column.label}</span>
            {:else if column.sort}
              <button type="button" class="sort" onclick={() => sortBy(column)}>
                {column.label}<span class="arrow" aria-hidden="true">{sorted === 'ascending' ? '↑' : sorted === 'descending' ? '↓' : ''}</span>
              </button>
            {:else}
              {column.label}
            {/if}
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each shown as row (key(row))}
        <tr>
          {#each columns as column (column.key)}
            <td class:end={column.align === 'end'} class:mono={column.mono} class:wrap={column.wrap}>
              {#if cell}{@render cell(row, column)}{:else}{text(row, column)}{/if}
            </td>
          {/each}
        </tr>
      {:else}
        <tr><td class="empty" colspan={columns.length}>{empty}</td></tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  /* a row is one line; a narrow screen scrolls the table inside its box, never the page. The box is positioned so
     the hidden caption and labels, which are absolute, stay inside it too. */
  .om-table {
    position: relative;
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  caption {
    padding-bottom: 8px;
    text-align: start;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
  }
  th {
    padding: 8px 12px 8px 0;
    text-align: start;
    font-size: 12px;
    font-weight: 400;
    color: var(--text-3);
    white-space: nowrap;
  }
  .sort {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    margin: -4px -6px;
    padding: 4px 6px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .sort:hover,
  [aria-sort] .sort {
    color: var(--text-1);
  }
  .arrow {
    display: inline-block;
    width: 1ch;
  }
  td {
    padding: 10px 12px 10px 0;
    border-top: 1px solid var(--line);
    color: var(--text-2);
    vertical-align: middle;
    white-space: nowrap;
  }
  .wrap {
    min-width: 8rem;
    white-space: normal;
  }
  .end {
    text-align: end;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  .empty {
    padding: 24px 0;
    text-align: center;
    color: var(--text-3);
  }
</style>
