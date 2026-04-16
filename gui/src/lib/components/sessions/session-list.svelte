<script lang="ts" module>
  import type { Alias } from '$lib/types';
  import type { AliasGroup } from '$lib/sessions-helpers';

  /** Flat row used for keyboard navigation and rendering. */
  export interface FlatRow {
    type: 'group' | 'subgroup' | 'alias';
    key: string;
    depth: number;
    group: AliasGroup;
    subgroupName?: string;
    alias?: Alias;
  }

  export function flatten(
    groups: AliasGroup[],
    collapsed: Record<string, boolean>
  ): FlatRow[] {
    const rows: FlatRow[] = [];
    for (const g of groups) {
      rows.push({ type: 'group', key: `g:${g.name}`, depth: 0, group: g });
      if (collapsed[g.name]) continue;
      for (const sg of g.subgroups) {
        rows.push({
          type: 'subgroup',
          key: `s:${g.name}:${sg.name}`,
          depth: 1,
          group: g,
          subgroupName: sg.name
        });
        for (const a of sg.aliases) {
          rows.push({
            type: 'alias',
            key: `a:${a.name}`,
            depth: 2,
            group: g,
            subgroupName: sg.name,
            alias: a
          });
        }
      }
    }
    return rows;
  }
</script>

<script lang="ts">
  import { tick } from 'svelte';
  import { ChevronDown, ChevronRight, Search } from 'lucide-svelte';
  import { Input } from '$lib/components/ui';
  import StatusDot from '$lib/components/status-dot.svelte';
  import {
    isActive,
    portHint,
    stateTone,
    subgroupIcon
  } from '$lib/sessions-helpers';
  import type { SessionStatus } from '$lib/types';
  import type { AliasGroup } from '$lib/sessions-helpers';
  import { cn } from '$lib/utils';

  interface Props {
    groups: AliasGroup[];
    sessions: Record<string, SessionStatus>;
    selectedAlias: string | null;
    filter: string;
    onSelect: (name: string) => void;
    onToggleGroup: (name: string) => void;
    collapsed: Record<string, boolean>;
    searchInput?: HTMLInputElement;
    totalCount: number;
  }

  let {
    groups,
    sessions,
    selectedAlias = $bindable(),
    filter = $bindable(),
    onSelect,
    onToggleGroup,
    collapsed,
    searchInput = $bindable(),
    totalCount
  }: Props = $props();

  let listEl: HTMLDivElement | null = $state(null);

  let rows = $derived(flatten(groups, collapsed));
  let aliasRows = $derived(rows.filter((r) => r.type === 'alias'));
  let visibleAliasCount = $derived(aliasRows.length);

  // Auto-select first alias when nothing selected
  $effect(() => {
    if (!selectedAlias && aliasRows.length > 0) {
      onSelect(aliasRows[0].alias!.name);
    }
  });

  // Scroll selected into view
  $effect(() => {
    if (!selectedAlias || !listEl) return;
    void rows;
    queueMicrotask(() => {
      const el = listEl?.querySelector<HTMLElement>(`[data-alias="${cssEscape(selectedAlias!)}"]`);
      el?.scrollIntoView({ block: 'nearest' });
    });
  });

  function cssEscape(s: string): string {
    return s.replace(/(["\\])/g, '\\$1');
  }
</script>

<div class="flex h-full flex-col">
  <div class="flex items-center gap-2 border-b border-border px-3 py-2">
    <div class="relative flex-1">
      <Search class="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input
        bind:value={filter}
        bind:ref={searchInput}
        placeholder="Filter aliases…  (press /)"
        class="h-8 pl-7 text-xs"
        spellcheck={false}
      />
    </div>
    <span class="shrink-0 text-[11px] tabular-nums text-muted-foreground">
      {visibleAliasCount}/{totalCount}
    </span>
  </div>

  <div bind:this={listEl} class="min-h-0 flex-1 overflow-auto py-1">
    {#each rows as row (row.key)}
      {#if row.type === 'group'}
        {@const GroupIcon = row.group.icon}
        {@const total = row.group.subgroups.reduce((acc, sg) => acc + sg.aliases.length, 0)}
        {@const isCollapsed = collapsed[row.group.name]}
        <button
          type="button"
          onclick={() => onToggleGroup(row.group.name)}
          class="flex w-full items-center gap-1.5 px-2 py-1.5 text-left text-xs font-semibold tracking-tight transition-colors hover:bg-accent/40"
        >
          {#if isCollapsed}
            <ChevronRight class="h-3 w-3 text-muted-foreground" />
          {:else}
            <ChevronDown class="h-3 w-3 text-muted-foreground" />
          {/if}
          <GroupIcon class="h-3.5 w-3.5 text-primary" />
          <span class="flex-1 truncate">{row.group.name}</span>
          <span class="rounded-full bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
            {total}
          </span>
        </button>
      {:else if row.type === 'subgroup'}
        {@const SubIcon = subgroupIcon(row.subgroupName!)}
        <div class="flex items-center gap-1.5 px-2 py-1 pl-7 text-[10px] uppercase tracking-wider text-muted-foreground">
          <SubIcon class="h-3 w-3" />
          <span class="truncate">{row.subgroupName}</span>
          <span class="h-px flex-1 bg-border/60"></span>
        </div>
      {:else}
        {@const a = row.alias!}
        {@const st = sessions[a.name]}
        {@const tone = stateTone(st?.state)}
        {@const selected = selectedAlias === a.name}
        {@const port = portHint(a)}
        <button
          type="button"
          data-alias={a.name}
          onclick={() => onSelect(a.name)}
          class={cn(
            'group flex w-full items-center gap-2 py-1 pl-3 pr-2 text-left text-xs transition-colors',
            selected
              ? 'bg-accent/70'
              : 'hover:bg-accent/30'
          )}
        >
          <span
            class={cn(
              'h-5 w-0.5 shrink-0 rounded-r',
              selected ? `bg-status-${tone === 'muted' ? 'info' : tone}` : 'bg-transparent'
            )}
          ></span>
          <span class="ml-1 shrink-0">
            <StatusDot tone={tone} pulse={st?.state === 'starting'} />
          </span>
          <span
            class={cn(
              'flex-1 truncate font-mono',
              selected
                ? 'font-semibold text-foreground'
                : isActive(st)
                  ? 'text-foreground'
                  : 'text-muted-foreground'
            )}
          >
            {a.name}
          </span>
          {#if port}
            <span class="shrink-0 font-mono text-[10px] text-muted-foreground/80">
              {port.length > 22 ? port.slice(0, 21) + '…' : port}
            </span>
          {:else if a.profile}
            <span class="shrink-0 font-mono text-[10px] text-muted-foreground/80">
              {a.profile}
            </span>
          {/if}
        </button>
      {/if}
    {/each}

    {#if rows.length === 0}
      <p class="px-4 py-8 text-center text-xs text-muted-foreground">
        {filter ? 'No aliases match' : 'No aliases loaded'}
      </p>
    {/if}
  </div>
</div>
