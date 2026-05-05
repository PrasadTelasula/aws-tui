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
  import { ArrowUp, ArrowDown, ArrowsDownUp as ArrowUpDown } from 'phosphor-svelte';

  interface Props {
    data: TData[];
    columns: Column<TData>[];
    filter?: string;
    class?: string;
    emptyLabel?: string;
    selectedKey?: string | number | null;
    onRowClick?: (row: TData) => void;
    rowKey?: (row: TData, index: number) => string | number;
  }

  let {
    data,
    columns,
    filter = '',
    class: className,
    emptyLabel = 'No results',
    selectedKey = null,
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
          const v = c.filterValue?.(r) ?? c.accessor?.(r) ?? defaultAccess(r, c.key);
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

<div class={cn('flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-border bg-card', className)}>
  <div class="min-h-0 flex-1 overflow-auto">
    <table class="w-full caption-bottom text-sm">
      <thead class="sticky top-0 z-10 bg-muted/60 backdrop-blur-sm [&_tr]:border-b">
        <tr class="border-b border-border">
          {#each columns as col (col.key)}
            <th
              class={cn(
                'h-10 px-3 text-left align-middle text-xs font-medium text-muted-foreground select-none',
                col.sortable && 'cursor-pointer hover:text-foreground',
                col.class
              )}
              onclick={() => toggleSort(col)}
            >
              <div class="flex items-center gap-1">
                <span>{col.header}</span>
                {#if col.sortable}
                  {#if sortKey === col.key}
                    {#if sortDir === 'asc'}
                      <ArrowUp class="h-3 w-3 text-primary" />
                    {:else}
                      <ArrowDown class="h-3 w-3 text-primary" />
                    {/if}
                  {:else}
                    <ArrowUpDown class="h-3 w-3 opacity-30" />
                  {/if}
                {/if}
              </div>
            </th>
          {/each}
        </tr>
      </thead>
      <tbody class="[&_tr:last-child]:border-0">
        {#each processed as row, i (rowKey ? rowKey(row, i) : i)}
          {@const key = rowKey ? rowKey(row, i) : i}
          {@const isSelected = selectedKey != null && key === selectedKey}
          <tr
            class={cn(
              'border-b border-border/60 transition-colors',
              onRowClick && 'cursor-pointer hover:bg-muted/40',
              isSelected && 'bg-primary/10 hover:bg-primary/[0.13]'
            )}
            onclick={() => onRowClick?.(row)}
          >
            {#each columns as col (col.key)}
              <td class={cn('px-3 py-2.5 align-middle', col.class)}>
                {#if col.cell}
                  {@render col.cell(row)}
                {:else}
                  <span class="text-sm">
                    {(col.accessor?.(row) ?? defaultAccess(row, col.key) ?? '—') as string}
                  </span>
                {/if}
              </td>
            {/each}
          </tr>
        {:else}
          <tr>
            <td colspan={columns.length} class="py-16 text-center text-sm text-muted-foreground">
              {emptyLabel}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>
