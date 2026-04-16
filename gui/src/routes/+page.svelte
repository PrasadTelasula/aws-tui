<script lang="ts">
  import { onMount } from 'svelte';
  import { createRawSnippet } from 'svelte';
  import { ipc } from '$lib/ipc';
  import type { Alias, SessionState, SessionStatus } from '$lib/types';
  import { aliases, sessions, loading } from '$lib/stores/aws';
  import PageHeader from '$lib/components/app-shell/page-header.svelte';
  import DataTable from '$lib/components/data-table.svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import { Badge, Button, Input, Kbd } from '$lib/components/ui';
  import { Play, RefreshCw, Search, Square } from 'lucide-svelte';
  import type { ColumnDef } from '@tanstack/svelte-table';

  let filter = $state('');

  async function refresh() {
    loading.update((l) => ({ ...l, aliases: true }));
    try {
      const list = await ipc.listAliases();
      aliases.set(list);
      const s = await ipc.listSessions();
      const byAlias: Record<string, SessionStatus> = {};
      for (const st of s) byAlias[st.alias] = st;
      sessions.set(byAlias);
    } finally {
      loading.update((l) => ({ ...l, aliases: false }));
    }
  }

  onMount(refresh);

  async function start(a: Alias) {
    const status = await ipc.startSession(a.name);
    sessions.update((s) => ({ ...s, [a.name]: status }));
  }

  async function stop(a: Alias) {
    const status = await ipc.stopSession(a.name);
    sessions.update((s) => ({ ...s, [a.name]: status }));
  }

  function stateTone(s: SessionState | undefined): 'ok' | 'warn' | 'error' | 'info' | 'muted' {
    switch (s) {
      case 'active': return 'ok';
      case 'starting': return 'info';
      case 'expired': return 'warn';
      case 'error': return 'error';
      default: return 'muted';
    }
  }

  function kindBadgeVariant(k: Alias['kind']): 'info' | 'ok' | 'warn' | 'muted' {
    switch (k) {
      case 'sso-login': return 'info';
      case 'ssm-session': return 'ok';
      case 'iam-profile': return 'warn';
      default: return 'muted';
    }
  }

  const columns: ColumnDef<Alias, any>[] = [
    {
      accessorKey: 'name',
      header: 'Alias',
      cell: (ctx) =>
        createRawSnippet(() => ({
          render: () => `<span class="font-mono text-sm font-medium">${ctx.getValue()}</span>`
        }))
    },
    {
      accessorKey: 'kind',
      header: 'Kind',
      cell: (ctx) => {
        const kind = ctx.getValue() as Alias['kind'];
        const variant = kindBadgeVariant(kind);
        return createRawSnippet(() => ({
          render: () =>
            `<span class="inline-flex items-center rounded-md px-2 py-0.5 text-xs font-medium bg-status-${
              variant === 'muted' ? 'info' : variant
            }/15 text-status-${variant === 'muted' ? 'info' : variant}">${kind}</span>`
        }));
      }
    },
    {
      accessorKey: 'profile',
      header: 'Profile',
      cell: (ctx) =>
        createRawSnippet(() => ({
          render: () =>
            `<span class="font-mono text-xs text-muted-foreground">${ctx.getValue() ?? '—'}</span>`
        }))
    },
    {
      accessorKey: 'region',
      header: 'Region',
      cell: (ctx) =>
        createRawSnippet(() => ({
          render: () =>
            `<span class="font-mono text-xs text-muted-foreground">${ctx.getValue() ?? '—'}</span>`
        }))
    },
    {
      id: 'status',
      header: 'Status',
      cell: (ctx) => {
        const a = ctx.row.original as Alias;
        const s = $sessions[a.name]?.state ?? 'idle';
        const tone = stateTone(s);
        return createRawSnippet(() => ({
          render: () =>
            `<span class="inline-flex items-center gap-2 text-xs"><span class="h-2 w-2 rounded-full bg-status-${
              tone === 'muted' ? 'info' : tone
            }"></span><span class="capitalize">${s}</span></span>`
        }));
      }
    }
  ];
</script>

<div class="space-y-4">
  <PageHeader
    title="Sessions"
    subtitle="Start, stop, and monitor AWS SSO, SSM, and IAM sessions defined in your shell aliases."
  >
    {#snippet actions()}
      <Button variant="outline" size="sm" onclick={refresh} disabled={$loading.aliases}>
        <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.aliases ? 'animate-spin' : '')} />
        Refresh
      </Button>
    {/snippet}
  </PageHeader>

  <div class="flex items-center gap-2">
    <div class="relative flex-1 max-w-sm">
      <Search class="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
      <Input class="pl-8" placeholder="Filter aliases…" bind:value={filter} />
    </div>
    <span class="text-xs text-muted-foreground">
      <Kbd>/</Kbd> to focus
    </span>
  </div>

  <DataTable
    data={$aliases}
    {columns}
    {filter}
    emptyLabel={$loading.aliases ? 'Loading aliases…' : 'No aliases found'}
  />

  <div class="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
    {#each $aliases.filter((a) => !filter || a.name.toLowerCase().includes(filter.toLowerCase())) as alias (alias.name)}
      {@const st = $sessions[alias.name]}
      {@const active = st?.state === 'active' || st?.state === 'starting'}
      <div class="rounded-lg border border-border bg-card p-4 transition-colors hover:border-primary/40">
        <div class="flex items-center justify-between gap-2">
          <div class="flex items-center gap-2">
            <StatusDot tone={stateTone(st?.state)} pulse={st?.state === 'starting'} />
            <span class="font-mono text-sm font-medium">{alias.name}</span>
          </div>
          <Badge variant={kindBadgeVariant(alias.kind)}>{alias.kind}</Badge>
        </div>
        <p class="mt-2 line-clamp-2 text-xs text-muted-foreground font-mono">{alias.command}</p>
        <div class="mt-3 flex items-center justify-between">
          <div class="flex items-center gap-2 text-xs text-muted-foreground">
            {#if alias.profile}<span class="font-mono">{alias.profile}</span>{/if}
            {#if alias.region}<span class="font-mono">· {alias.region}</span>{/if}
          </div>
          {#if active}
            <Button variant="destructive" size="sm" onclick={() => stop(alias)}>
              <Square class="h-3.5 w-3.5" /> Stop
            </Button>
          {:else}
            <Button size="sm" onclick={() => start(alias)}>
              <Play class="h-3.5 w-3.5" /> Start
            </Button>
          {/if}
        </div>
      </div>
    {/each}
  </div>
</div>
