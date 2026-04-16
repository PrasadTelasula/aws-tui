<script lang="ts">
  import { onMount } from 'svelte';
  import { ipc } from '$lib/ipc';
  import type { Alias, SessionState, SessionStatus } from '$lib/types';
  import { aliases, sessions, loading } from '$lib/stores/aws';
  import PageHeader from '$lib/components/app-shell/page-header.svelte';
  import DataTable, { type Column } from '$lib/components/data-table.svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import { Badge, Button, Input, Kbd } from '$lib/components/ui';
  import { Play, RefreshCw, Search, Square } from 'lucide-svelte';

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

  let visible = $derived(
    $aliases.filter((a) => !filter || a.name.toLowerCase().includes(filter.toLowerCase()))
  );

  const columns: Column<Alias>[] = [
    { key: 'name', header: 'Alias', sortable: true, accessor: (r) => r.name },
    { key: 'kind', header: 'Kind', sortable: true, accessor: (r) => r.kind },
    { key: 'profile', header: 'Profile', sortable: true, accessor: (r) => r.profile ?? '' },
    { key: 'region', header: 'Region', sortable: true, accessor: (r) => r.region ?? '' },
    { key: 'status', header: 'Status' }
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
    rowKey={(r) => r.name}
    emptyLabel={$loading.aliases ? 'Loading aliases…' : 'No aliases found'}
  />

  <div class="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
    {#each visible as alias (alias.name)}
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
