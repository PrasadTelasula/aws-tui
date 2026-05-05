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
  import { CaretDown, MagnifyingGlass } from 'phosphor-svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import {
    aliasMeta,
    isActive,
    portHint,
    stateTone
  } from '$lib/sessions-helpers';
  import type { SessionStatus } from '$lib/types';
  import { uptimeFrom } from '$lib/utils';

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
    nowTick: number;
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
    totalCount,
    nowTick
  }: Props = $props();

  let listEl: HTMLDivElement | null = $state(null);

  let rows = $derived(flatten(groups, collapsed));
  let aliasRows = $derived(rows.filter((r) => r.type === 'alias'));
  let visibleAliasCount = $derived(aliasRows.length);
  let runningCount = $derived(
    Object.values(sessions).filter((s) => isActive(s)).length
  );

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

  function stateBadgeClass(state: SessionStatus['state'] | undefined): string | null {
    if (!state) return null;
    if (state === 'running' || state === 'connected') return 'is-ok';
    if (state === 'starting') return 'is-info';
    if (state === 'expired') return 'is-warn';
    if (state === 'error') return 'is-err';
    return null;
  }

  function stateBadgeText(state: SessionStatus['state'] | undefined): string | null {
    if (!state) return null;
    if (state === 'running' || state === 'connected') return 'active';
    if (state === 'starting') return 'starting';
    if (state === 'expired') return 'expired';
    if (state === 'error') return 'error';
    return null;
  }

  function subline(a: Alias, st: SessionStatus | undefined, now: number): string {
    if (st && isActive(st) && st.startedAt) {
      void now;
      return `${a.command.split(/\s+/)[0] ?? a.kind} · up ${uptimeFrom(st.startedAt)}`;
    }
    if (a.profile) return `${a.kind} · ${a.profile}`;
    const port = portHint(a);
    return port ?? a.command;
  }
</script>

<div class="tui-split-list">
  <div class="tui-split-list-header">
    <div class="tui-search">
      <span class="tui-search-icon"><Search size={13} weight="regular" /></span>
      <input
        bind:this={searchInput}
        bind:value={filter}
        class="tui-search-input"
        placeholder="Filter aliases…"
        spellcheck={false}
      />
      <span class="tui-search-kbd"><kbd class="tui-kbd">/</kbd></span>
    </div>
    <div class="tui-split-list-meta">
      <span>{visibleAliasCount} of {totalCount}</span>
      <span class="tui-split-list-meta-mono">
        {runningCount > 0 ? `${runningCount} running` : 'all idle'}
      </span>
    </div>
  </div>

  <div bind:this={listEl} class="tui-split-list-body">
    {#each rows as row (row.key)}
      {#if row.type === 'group'}
        {@const total = row.group.subgroups.reduce((acc, sg) => acc + sg.aliases.length, 0)}
        {@const isCollapsed = !!collapsed[row.group.name]}
        <button
          type="button"
          class="tui-group-header"
          class:is-collapsed={isCollapsed}
          onclick={() => onToggleGroup(row.group.name)}
        >
          <span class="tui-group-header-chev"><ChevronDown size={11} weight="bold" /></span>
          <span>{row.group.name}</span>
          <span class="tui-group-header-line"></span>
          <span class="tui-group-header-count">{total}</span>
        </button>
      {:else if row.type === 'subgroup'}
        <div class="tui-subgroup">
          <span>{row.subgroupName}</span>
          <span class="tui-subgroup-line"></span>
        </div>
      {:else}
        {@const a = row.alias!}
        {@const st = sessions[a.name]}
        {@const tone = stateTone(st?.state)}
        {@const selected = selectedAlias === a.name}
        {@const km = aliasMeta(a)}
        {@const KindIcon = km.Icon}
        {@const badgeClass = stateBadgeClass(st?.state)}
        {@const badgeText = stateBadgeText(st?.state)}
        {@const active = isActive(st)}
        <button
          type="button"
          data-alias={a.name}
          class="tui-alias-row tui-alias-row-rich"
          class:is-selected={selected}
          class:is-active={active}
          onclick={() => onSelect(a.name)}
        >
          <span class="tui-alias-row-kind">
            <span class={`tui-kind tui-kind-${km.tone} tui-kind-compact`} title={km.label}>
              <KindIcon size={11} weight="bold" />
            </span>
          </span>
          <span class="tui-alias-row-body">
            <span class="tui-alias-row-line1">
              <StatusDot tone={tone} pulse={st?.state === 'starting'} size={6} />
              <span class="tui-alias-name">{a.name}</span>
              {#if badgeText}
                <span class={`tui-alias-row-state ${badgeClass}`}>{badgeText}</span>
              {/if}
            </span>
            <span class="tui-alias-row-line2" title={a.command}>
              {subline(a, st, nowTick)}
            </span>
          </span>
        </button>
      {/if}
    {/each}

    {#if rows.length === 0}
      <p style="padding: 24px 16px; text-align: center; color: var(--tui-fg-4); font-size: 12px;">
        {filter ? 'No aliases match' : 'No aliases loaded'}
      </p>
    {/if}
  </div>
</div>
