<script lang="ts" module>
  import type { Snippet } from 'svelte';

  export interface Column<TData> {
    key: string;
    header: string;
    accessor?: (row: TData) => unknown;
    cell?: Snippet<[TData]>;
    sortable?: boolean;
    sortValue?: (row: TData) => string | number;
    filterValue?: (row: TData) => string;
    class?: string;
  }
</script>

<script lang="ts" generics="TData">
  import { cn } from '$lib/utils';

  interface Props {
    data: TData[];
    columns: Column<TData>[];
    filter?: string;
    class?: string;
    emptyLabel?: string;
    onRowClick?: (row: TData) => void;
    rowKey?: (row: TData, index: number) => string | number;
  }

  let {
    data,
    columns,
    filter = '',
    class: className,
    emptyLabel = 'No results',
    onRowClick,
    rowKey
  }: Props = $props();

  let sortKey = $state<string | null>(null);
  let sortDir = $state<'asc' | 'desc'>('asc');

  function toggleSort(col: Column<TData>) {
    if (!col.sortable) return;
    if (sortKey === col.key) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortKey = col.key;
      sortDir = 'asc';
    }
  }

  function defaultAccess(row: TData, key: string): unknown {
    return (row as Record<string, unknown>)[key];
  }

  let processed = $derived.by(() => {
    let rows = data;
    const f = filter.trim().toLowerCase();
    if (f) {
      rows = rows.filter((r) =>
        columns.some((c) => {
          const v =
            c.filterValue?.(r) ?? c.accessor?.(r) ?? defaultAccess(r, c.key);
          return v != null && String(v).toLowerCase().includes(f);
        })
      );
    }
    if (sortKey) {
      const col = columns.find((c) => c.key === sortKey);
      if (col) {
        const get = (r: TData) =>
          col.sortValue?.(r) ??
          (col.accessor?.(r) as string | number | null | undefined) ??
          (defaultAccess(r, col.key) as string | number | null | undefined) ??
          '';
        rows = [...rows].sort((a, b) => {
          const av = get(a);
          const bv = get(b);
          if (av === bv) return 0;
          const cmp = av < bv ? -1 : 1;
          return sortDir === 'asc' ? cmp : -cmp;
        });
      }
    }
    return rows;
  });
</script>

<div class={cn('overflow-hidden rounded-lg border border-border bg-card', className)}>
  <table class="w-full text-sm">
    <thead class="bg-muted/40">
      <tr>
        {#each columns as col (col.key)}
          <th
            class={cn(
              'h-10 px-4 text-left align-middle font-medium text-muted-foreground',
              col.sortable && 'cursor-pointer select-none',
              col.class
            )}
            onclick={() => toggleSort(col)}
          >
            <div class="flex items-center gap-1.5">
              <span>{col.header}</span>
              {#if col.sortable && sortKey === col.key}
                <span class="text-xs">{sortDir === 'asc' ? '↑' : '↓'}</span>
              {/if}
            </div>
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each processed as row, i (rowKey ? rowKey(row, i) : i)}
        <tr
          class={cn(
            'border-t border-border transition-colors hover:bg-muted/40',
            onRowClick && 'cursor-pointer'
          )}
          onclick={() => onRowClick?.(row)}
        >
          {#each columns as col (col.key)}
            <td class={cn('px-4 py-2.5 align-middle', col.class)}>
              {#if col.cell}
                {@render col.cell(row)}
              {:else}
                <span>{(col.accessor?.(row) ?? defaultAccess(row, col.key) ?? '—') as string}</span>
              {/if}
            </td>
          {/each}
        </tr>
      {:else}
        <tr>
          <td colspan={columns.length} class="py-10 text-center text-sm text-muted-foreground">
            {emptyLabel}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
