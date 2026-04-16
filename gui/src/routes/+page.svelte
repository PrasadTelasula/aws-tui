<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { tick } from 'svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { ipc } from '$lib/ipc';
  import type { Alias, SessionState, SessionStatus } from '$lib/types';
  import { aliases, aliasesPath, sessions, loading } from '$lib/stores/aws';
  import PageHeader from '$lib/components/app-shell/page-header.svelte';
  import DataTable, { type Column } from '$lib/components/data-table.svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import { Badge, Button, Input, Kbd } from '$lib/components/ui';
  import { Play, RefreshCw, Search, Square, Terminal as TermIcon, X } from 'lucide-svelte';

  let filter = $state('');
  let loadError = $state<string | null>(null);
  let viewing = $state<string | null>(null);
  let viewingLines = $state<string[]>([]);
  let outputBox: HTMLDivElement | null = $state(null);

  const unlistens: Map<string, UnlistenFn[]> = new Map();

  async function refresh() {
    loading.update((l) => ({ ...l, aliases: true }));
    loadError = null;
    try {
      const resp = await ipc.listAliases();
      aliases.set(resp.aliases);
      aliasesPath.set(resp.path);
      const s = await ipc.listSessions();
      const byAlias: Record<string, SessionStatus> = {};
      for (const st of s) byAlias[st.alias] = st;
      sessions.set(byAlias);
    } catch (e) {
      loadError = String(e);
      aliases.set([]);
    } finally {
      loading.update((l) => ({ ...l, aliases: false }));
    }
  }

  onMount(refresh);

  onDestroy(() => {
    for (const fns of unlistens.values()) for (const fn of fns) fn();
    unlistens.clear();
  });

  async function attachListeners(alias: string) {
    if (unlistens.has(alias)) return;
    const onOut = await ipc.onSessionOutput(alias, (line) => {
      if (viewing === alias) {
        viewingLines = [...viewingLines, line].slice(-500);
        scrollOutputToBottom();
      }
    });
    const onStat = await ipc.onSessionStatus(alias, (status) => {
      sessions.update((s) => ({ ...s, [alias]: status }));
    });
    unlistens.set(alias, [onOut, onStat]);
  }

  function detachListeners(alias: string) {
    const fns = unlistens.get(alias);
    if (fns) {
      for (const fn of fns) fn();
      unlistens.delete(alias);
    }
  }

  async function start(a: Alias) {
    await attachListeners(a.name);
    try {
      const status = await ipc.startSession(a.name, a.command);
      sessions.update((s) => ({ ...s, [a.name]: status }));
    } catch (e) {
      loadError = `Failed to start ${a.name}: ${e}`;
    }
  }

  async function stop(a: Alias) {
    try {
      const status = await ipc.stopSession(a.name);
      sessions.update((s) => ({ ...s, [a.name]: status }));
    } finally {
      detachListeners(a.name);
    }
  }

  async function viewOutput(a: Alias) {
    viewing = a.name;
    viewingLines = await ipc.sessionOutput(a.name);
    await attachListeners(a.name);
    await tick();
    scrollOutputToBottom();
  }

  function closeOutput() {
    viewing = null;
    viewingLines = [];
  }

  function scrollOutputToBottom() {
    if (outputBox) outputBox.scrollTop = outputBox.scrollHeight;
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
    subtitle={$aliasesPath
      ? `Loaded from ${$aliasesPath}`
      : 'Start, stop, and monitor AWS SSO, SSM, and IAM sessions defined in your shell aliases.'}
  >
    {#snippet actions()}
      <Button variant="outline" size="sm" onclick={refresh} disabled={$loading.aliases}>
        <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.aliases ? 'animate-spin' : '')} />
        Refresh
      </Button>
    {/snippet}
  </PageHeader>

  {#if loadError}
    <div class="rounded-md border border-status-error/30 bg-status-error/10 px-3 py-2 text-sm text-status-error">
      {loadError}
    </div>
  {/if}

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
            {#if st?.pid}<span class="font-mono">· pid {st.pid}</span>{/if}
          </div>
          <div class="flex items-center gap-1.5">
            {#if active}
              <Button variant="ghost" size="sm" onclick={() => viewOutput(alias)}>
                <TermIcon class="h-3.5 w-3.5" />
              </Button>
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
      </div>
    {/each}
  </div>
</div>

{#if viewing}
  <div
    role="dialog"
    aria-modal="true"
    class="fixed inset-0 z-50 flex items-end justify-center bg-black/40 backdrop-blur-sm sm:items-center sm:p-4"
    onclick={closeOutput}
  >
    <div
      class="flex h-[70vh] w-full max-w-3xl flex-col overflow-hidden rounded-t-lg border border-border bg-card shadow-xl sm:rounded-lg"
      onclick={(e) => e.stopPropagation()}
    >
      <div class="flex items-center justify-between border-b border-border px-4 py-2.5">
        <div class="flex items-center gap-2">
          <TermIcon class="h-4 w-4 text-primary" />
          <span class="font-mono text-sm font-medium">{viewing}</span>
          <Badge variant={stateTone($sessions[viewing]?.state)}>
            {$sessions[viewing]?.state ?? 'idle'}
          </Badge>
        </div>
        <button
          onclick={closeOutput}
          class="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
          aria-label="Close"
        >
          <X class="h-4 w-4" />
        </button>
      </div>
      <div bind:this={outputBox} class="min-h-0 flex-1 overflow-auto bg-[#0f1114] p-3 font-mono text-xs text-[#d4d4d4]">
        {#if viewingLines.length === 0}
          <span class="text-muted-foreground italic">(no output yet)</span>
        {:else}
          {#each viewingLines as line, i (i)}
            <div class="whitespace-pre-wrap leading-relaxed">{line}</div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}
