<script lang="ts" generics="TData">
  import {
    createSvelteTable,
    flexRender,
    getCoreRowModel,
    getFilteredRowModel,
    getSortedRowModel,
    type ColumnDef,
    type SortingState
  } from '@tanstack/svelte-table';
  import { cn } from '$lib/utils';

  interface Props {
    data: TData[];
    columns: ColumnDef<TData, any>[];
    filter?: string;
    class?: string;
    emptyLabel?: string;
    onRowClick?: (row: TData) => void;
  }

  let {
    data,
    columns,
    filter = '',
    class: className,
    emptyLabel = 'No results',
    onRowClick
  }: Props = $props();

  let sorting = $state<SortingState>([]);

  const table = createSvelteTable<TData>({
    get data() {
      return data;
    },
    columns,
    state: {
      get sorting() {
        return sorting;
      },
      get globalFilter() {
        return filter;
      }
    },
    onSortingChange: (updater) => {
      sorting = typeof updater === 'function' ? updater(sorting) : updater;
    },
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel()
  });
</script>

<div class={cn('overflow-hidden rounded-lg border border-border bg-card', className)}>
  <table class="w-full text-sm">
    <thead class="bg-muted/40">
      {#each table.getHeaderGroups() as group (group.id)}
        <tr>
          {#each group.headers as header (header.id)}
            <th
              class={cn(
                'h-10 px-4 text-left align-middle font-medium text-muted-foreground',
                header.column.getCanSort() && 'cursor-pointer select-none'
              )}
              onclick={header.column.getCanSort()
                ? header.column.getToggleSortingHandler()
                : undefined}
            >
              <div class="flex items-center gap-1.5">
                {#if !header.isPlaceholder}
                  <svelte:component
                    this={flexRender(header.column.columnDef.header, header.getContext())}
                  />
                {/if}
                {#if header.column.getIsSorted() === 'asc'}
                  <span class="text-xs">↑</span>
                {:else if header.column.getIsSorted() === 'desc'}
                  <span class="text-xs">↓</span>
                {/if}
              </div>
            </th>
          {/each}
        </tr>
      {/each}
    </thead>
    <tbody>
      {#each table.getRowModel().rows as row (row.id)}
        <tr
          class={cn(
            'border-t border-border transition-colors hover:bg-muted/40',
            onRowClick && 'cursor-pointer'
          )}
          onclick={() => onRowClick?.(row.original)}
        >
          {#each row.getVisibleCells() as cell (cell.id)}
            <td class="px-4 py-2.5 align-middle">
              <svelte:component
                this={flexRender(cell.column.columnDef.cell, cell.getContext())}
              />
            </td>
          {/each}
        </tr>
      {:else}
        <tr>
          <td
            colspan={columns.length}
            class="py-10 text-center text-sm text-muted-foreground"
          >
            {emptyLabel}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
