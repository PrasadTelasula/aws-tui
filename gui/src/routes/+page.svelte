<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { ipc } from '$lib/ipc';
  import type { Alias, SessionStatus } from '$lib/types';
  import { aliases, aliasesPath, sessions, loading } from '$lib/stores/aws';
  import {
    groupAliases,
    isActive,
    kindBadgeVariant,
    kindLabel,
    outputLineClass,
    portHint,
    stateLabel,
    stateTone,
    subgroupIcon
  } from '$lib/sessions-helpers';
  import { formatDuration, uptimeFrom } from '$lib/utils';
  import PageHeader from '$lib/components/app-shell/page-header.svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import CredentialsModal from '$lib/components/credentials-modal.svelte';
  import ConfirmModal from '$lib/components/confirm-modal.svelte';
  import { Badge, Button, Input, Kbd } from '$lib/components/ui';
  import {
    KeyRound,
    Play,
    PowerOff,
    RefreshCw,
    Search,
    Square,
    Terminal as TermIcon,
    X,
    ChevronDown,
    ChevronRight
  } from 'lucide-svelte';

  let filter = $state('');
  let loadError = $state<string | null>(null);
  let viewing = $state<string | null>(null);
  let viewingLines = $state<string[]>([]);
  let outputBox: HTMLDivElement | null = $state(null);
  let credentialsFor = $state<string | null>(null);
  let confirmStopAll = $state(false);
  let collapsed = $state<Record<string, boolean>>({});
  let now = $state(Date.now());

  const unlistens: Map<string, UnlistenFn[]> = new Map();
  let unlistenChanged: UnlistenFn | null = null;
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  async function refresh() {
    loading.update((l) => ({ ...l, aliases: true }));
    loadError = null;
    try {
      const resp = await ipc.listAliases();
      aliases.set(resp.aliases);
      aliasesPath.set(resp.path);
      await syncSessions();
      verifyExistingInBackground(resp.aliases);
    } catch (e) {
      loadError = String(e);
      aliases.set([]);
    } finally {
      loading.update((l) => ({ ...l, aliases: false }));
    }
  }

  async function syncSessions() {
    const list = await ipc.listSessions();
    const byAlias: Record<string, SessionStatus> = {};
    for (const st of list) byAlias[st.alias] = st;
    sessions.set(byAlias);
  }

  async function verifyExistingInBackground(list: Alias[]) {
    const ssoPairs: Array<[string, string]> = list
      .filter((a) => a.kind === 'sso-login' && a.ssoSessionName)
      .map((a) => [a.name, a.ssoSessionName!]);
    const iamPairs: Array<[string, string]> = list
      .filter((a) => a.kind === 'iam-profile')
      .map((a) => [a.name, a.name]);
    try {
      if (ssoPairs.length) await ipc.checkExistingSso(ssoPairs);
      if (iamPairs.length) await ipc.checkExistingIam(iamPairs);
    } catch (e) {
      console.warn('startup verification failed', e);
    }
    await syncSessions();
  }

  onMount(async () => {
    await refresh();
    unlistenChanged = await ipc.onSessionsChanged(() => syncSessions());
    pollTimer = setInterval(() => {
      now = Date.now();
    }, 1000);
  });

  onDestroy(() => {
    for (const fns of unlistens.values()) for (const fn of fns) fn();
    unlistens.clear();
    unlistenChanged?.();
    if (pollTimer) clearInterval(pollTimer);
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
      const status = await ipc.startSession(
        a.name,
        a.command,
        a.kind,
        a.ssoSessionName,
        a.profile
      );
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

  async function doStopAll() {
    confirmStopAll = false;
    try {
      const n = await ipc.stopAllSessions();
      loadError = null;
      console.log(`Stopped ${n} session(s)`);
      await syncSessions();
    } catch (e) {
      loadError = `stop-all failed: ${e}`;
    }
  }

  function toggleGroup(name: string) {
    collapsed = { ...collapsed, [name]: !collapsed[name] };
  }

  function matchesFilter(a: Alias): boolean {
    if (!filter) return true;
    const f = filter.toLowerCase();
    return (
      a.name.toLowerCase().includes(f) ||
      (a.profile?.toLowerCase().includes(f) ?? false) ||
      (a.region?.toLowerCase().includes(f) ?? false) ||
      (a.subgroup?.toLowerCase().includes(f) ?? false) ||
      (a.group?.toLowerCase().includes(f) ?? false) ||
      a.command.toLowerCase().includes(f)
    );
  }

  let groups = $derived(
    groupAliases($aliases.filter(matchesFilter))
  );

  let runningCount = $derived(
    Object.values($sessions).filter((s) => isActive(s)).length
  );

  function expiryHint(s: SessionStatus | undefined): string | null {
    if (!s) return null;
    if (s.tokenRemainingSecs == null) return null;
    if (s.tokenRemainingSecs === 0) return 'expired';
    return `expires in ${formatDuration(s.tokenRemainingSecs)}`;
  }

  function uptime(s: SessionStatus | undefined): string {
    void now;
    return uptimeFrom(s?.startedAt ?? null);
  }
</script>

<div class="space-y-4">
  <PageHeader
    title="Sessions"
    subtitle={$aliasesPath
      ? `Loaded from ${$aliasesPath} · ${runningCount} active`
      : 'Start, stop, and monitor AWS SSO, SSM, and IAM sessions defined in your shell aliases.'}
  >
    {#snippet actions()}
      {#if runningCount > 0}
        <Button variant="outline" size="sm" onclick={() => (confirmStopAll = true)}>
          <PowerOff class="h-3.5 w-3.5" /> Stop all ({runningCount})
        </Button>
      {/if}
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
      {$aliases.length} alias{$aliases.length === 1 ? '' : 'es'}
    </span>
  </div>

  <div class="space-y-5">
    {#each groups as g (g.name)}
      {@const isCollapsed = collapsed[g.name]}
      {@const GroupIcon = g.icon}
      {@const total = g.subgroups.reduce((acc, sg) => acc + sg.aliases.length, 0)}
      <section>
        <button
          type="button"
          onclick={() => toggleGroup(g.name)}
          class="flex w-full items-center gap-2 py-1 text-left"
        >
          {#if isCollapsed}
            <ChevronRight class="h-4 w-4 text-muted-foreground" />
          {:else}
            <ChevronDown class="h-4 w-4 text-muted-foreground" />
          {/if}
          <GroupIcon class="h-4 w-4 text-primary" />
          <h3 class="text-sm font-semibold tracking-tight">{g.name}</h3>
          <span class="rounded-full bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
            {total}
          </span>
          {#if !g.explicit}
            <span class="text-[10px] uppercase tracking-wider text-muted-foreground">auto</span>
          {/if}
        </button>

        {#if !isCollapsed}
          <div class="mt-2 space-y-4">
            {#each g.subgroups as sg (sg.name)}
              {@const SubIcon = subgroupIcon(sg.name)}
              <div>
                <div class="mb-2 flex items-center gap-2 px-1 text-xs uppercase tracking-wider text-muted-foreground">
                  <SubIcon class="h-3 w-3" />
                  <span>{sg.name}</span>
                  <span class="text-[10px] normal-case text-muted-foreground/60">({sg.aliases.length})</span>
                  <span class="h-px flex-1 bg-border"></span>
                </div>
                <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                  {#each sg.aliases as alias (alias.name)}
                    {@const st = $sessions[alias.name]}
                    {@const active = isActive(st)}
                    {@const port = portHint(alias)}
                    {@const exp = expiryHint(st)}
                    <div class="rounded-lg border border-border bg-card p-4 transition-colors hover:border-primary/40">
                      <div class="flex items-center justify-between gap-2">
                        <div class="flex min-w-0 items-center gap-2">
                          <StatusDot
                            tone={stateTone(st?.state)}
                            pulse={st?.state === 'starting'}
                          />
                          <span class="truncate font-mono text-sm font-medium">{alias.name}</span>
                        </div>
                        <Badge variant={kindBadgeVariant(alias.kind)}>{kindLabel(alias.kind)}</Badge>
                      </div>

                      {#if port}
                        <p class="mt-2 truncate font-mono text-xs text-muted-foreground">{port}</p>
                      {:else}
                        <p class="mt-2 line-clamp-2 font-mono text-xs text-muted-foreground">{alias.command}</p>
                      {/if}

                      <div class="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                        <span class="capitalize">{stateLabel(st?.state)}</span>
                        {#if st?.pid}<span class="font-mono">· pid {st.pid}</span>{/if}
                        {#if active}<span>· up {uptime(st)}</span>{/if}
                        {#if alias.profile}<span class="font-mono">· {alias.profile}</span>{/if}
                        {#if alias.region}<span class="font-mono">· {alias.region}</span>{/if}
                        {#if exp}
                          <span class={st?.tokenRemainingSecs === 0 ? 'text-status-warn' : ''}>
                            · {exp}
                          </span>
                        {/if}
                      </div>

                      {#if st?.errorMessage}
                        <p class="mt-2 truncate text-xs text-status-error" title={st.errorMessage}>
                          {st.errorMessage}
                        </p>
                      {/if}

                      <div class="mt-3 flex items-center justify-end gap-1.5">
                        {#if st?.hasCredentials}
                          <Button
                            variant="ghost"
                            size="sm"
                            onclick={() => (credentialsFor = alias.name)}
                          >
                            <KeyRound class="h-3.5 w-3.5" /> Creds
                          </Button>
                        {/if}
                        {#if active || (st?.output ?? false)}
                          <Button variant="ghost" size="sm" onclick={() => viewOutput(alias)}>
                            <TermIcon class="h-3.5 w-3.5" />
                          </Button>
                        {/if}
                        {#if active}
                          <Button variant="destructive" size="sm" onclick={() => stop(alias)}>
                            <Square class="h-3.5 w-3.5" /> Stop
                          </Button>
                        {:else}
                          <Button size="sm" onclick={() => start(alias)}>
                            <Play class="h-3.5 w-3.5" />
                            {st?.state === 'expired' ? 'Re-login' : 'Start'}
                          </Button>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {:else}
      <div class="rounded-lg border border-dashed border-border p-10 text-center text-sm text-muted-foreground">
        {$loading.aliases ? 'Loading aliases…' : filter ? 'No aliases match your filter' : 'No aliases'}
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
            {stateLabel($sessions[viewing]?.state)}
          </Badge>
          {#if $sessions[viewing]?.pid}
            <span class="font-mono text-xs text-muted-foreground">
              pid {$sessions[viewing]?.pid}
            </span>
          {/if}
        </div>
        <button
          onclick={closeOutput}
          class="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
          aria-label="Close"
        >
          <X class="h-4 w-4" />
        </button>
      </div>
      <div bind:this={outputBox} class="min-h-0 flex-1 overflow-auto bg-[#0f1114] p-3 font-mono text-xs">
        {#if viewingLines.length === 0}
          <span class="italic text-muted-foreground">(no output yet)</span>
        {:else}
          {#each viewingLines as line, i (i)}
            <div class={'whitespace-pre-wrap leading-relaxed ' + outputLineClass(line)}>{line}</div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if credentialsFor}
  <CredentialsModal
    alias={credentialsFor}
    status={$sessions[credentialsFor]}
    onClose={() => (credentialsFor = null)}
  />
{/if}

{#if confirmStopAll}
  <ConfirmModal
    title="Stop all sessions"
    message={`Stop ${runningCount} active session${runningCount === 1 ? '' : 's'}? Running processes will be terminated and SSO logins dismissed.`}
    confirmLabel="Stop all"
    danger
    onConfirm={doStopAll}
    onCancel={() => (confirmStopAll = false)}
  />
{/if}
