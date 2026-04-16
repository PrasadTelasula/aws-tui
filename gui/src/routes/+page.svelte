<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { ipc } from '$lib/ipc';
  import type { Alias, SessionStatus } from '$lib/types';
  import { aliases, aliasesPath, sessions, loading } from '$lib/stores/aws';
  import { groupAliases, isActive } from '$lib/sessions-helpers';
  import type { AliasGroup } from '$lib/sessions-helpers';
  import { flatten } from '$lib/components/sessions/session-list.svelte';
  import SessionList from '$lib/components/sessions/session-list.svelte';
  import SessionDetail from '$lib/components/sessions/session-detail.svelte';
  import CredentialsModal from '$lib/components/credentials-modal.svelte';
  import ConfirmModal from '$lib/components/confirm-modal.svelte';
  import { Button, Kbd } from '$lib/components/ui';
  import { PowerOff, RefreshCw } from 'lucide-svelte';

  let filter = $state('');
  let loadError = $state<string | null>(null);
  let selectedAlias = $state<string | null>(null);
  let selectedOutput = $state<string[]>([]);
  let credentialsFor = $state<string | null>(null);
  let confirmStopAll = $state(false);
  let collapsed = $state<Record<string, boolean>>({});
  let now = $state(Date.now());
  let searchInput: HTMLInputElement | undefined = $state();

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
      .map((a) => [a.name, a.profile ?? a.name]);
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
    window.addEventListener('keydown', onKeydown);
  });

  onDestroy(() => {
    for (const fns of unlistens.values()) for (const fn of fns) fn();
    unlistens.clear();
    unlistenChanged?.();
    if (pollTimer) clearInterval(pollTimer);
    window.removeEventListener('keydown', onKeydown);
  });

  async function attachListeners(alias: string) {
    if (unlistens.has(alias)) return;
    const onOut = await ipc.onSessionOutput(alias, (line) => {
      if (selectedAlias === alias) {
        selectedOutput = [...selectedOutput, line].slice(-1000);
      }
    });
    const onStat = await ipc.onSessionStatus(alias, (status) => {
      sessions.update((s) => ({ ...s, [alias]: status }));
    });
    unlistens.set(alias, [onOut, onStat]);
  }

  async function selectAlias(name: string) {
    if (selectedAlias === name) return;
    selectedAlias = name;
    selectedOutput = [];
    await attachListeners(name);
    try {
      selectedOutput = await ipc.sessionOutput(name);
    } catch {
      selectedOutput = [];
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
    } catch (e) {
      loadError = `Failed to stop ${a.name}: ${e}`;
    }
  }

  async function doStopAll() {
    confirmStopAll = false;
    try {
      await ipc.stopAllSessions();
      await syncSessions();
    } catch (e) {
      loadError = `stop-all failed: ${e}`;
    }
  }

  async function copyCommand(cmd: string) {
    try {
      const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
      await writeText(cmd);
    } catch {
      try {
        await navigator.clipboard.writeText(cmd);
      } catch {
        // ignore
      }
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

  let groups: AliasGroup[] = $derived(groupAliases($aliases.filter(matchesFilter)));
  let runningCount = $derived(
    Object.values($sessions).filter((s) => isActive(s)).length
  );
  let selectedAliasObj = $derived(
    selectedAlias ? $aliases.find((a) => a.name === selectedAlias) ?? null : null
  );

  // ─── Keyboard shortcuts ─────────────────────────────────────────────

  function onKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement | null)?.tagName ?? '';
    const inInput = ['INPUT', 'TEXTAREA'].includes(tag);

    // Global: '/' focuses search (works even when input has focus)
    if (e.key === '/' && !inInput) {
      e.preventDefault();
      searchInput?.focus();
      searchInput?.select();
      return;
    }

    // Esc clears selection state
    if (e.key === 'Escape') {
      if (credentialsFor) { credentialsFor = null; return; }
      if (confirmStopAll) { confirmStopAll = false; return; }
      if (inInput && document.activeElement === searchInput) {
        if (filter) { filter = ''; return; }
        searchInput?.blur();
        return;
      }
    }

    if (inInput) return;

    const flat = flatten(groups, collapsed).filter((r) => r.type === 'alias');
    if (flat.length === 0) return;
    const currentIdx = flat.findIndex((r) => r.alias!.name === selectedAlias);
    const cur = currentIdx >= 0 ? currentIdx : 0;

    if (e.key === 'ArrowDown' || e.key === 'j') {
      e.preventDefault();
      const next = flat[Math.min(cur + 1, flat.length - 1)];
      if (next.alias) selectAlias(next.alias.name);
      return;
    }
    if (e.key === 'ArrowUp' || e.key === 'k') {
      e.preventDefault();
      const prev = flat[Math.max(cur - 1, 0)];
      if (prev.alias) selectAlias(prev.alias.name);
      return;
    }
    if (e.key === 'Home' || e.key === 'g') {
      e.preventDefault();
      if (flat[0]?.alias) selectAlias(flat[0].alias.name);
      return;
    }
    if (e.key === 'End' || e.key === 'G') {
      e.preventDefault();
      const last = flat[flat.length - 1];
      if (last.alias) selectAlias(last.alias.name);
      return;
    }
    if (e.key === 'Enter') {
      if (selectedAliasObj && !isActive($sessions[selectedAliasObj.name])) {
        e.preventDefault();
        start(selectedAliasObj);
      }
      return;
    }
    if (e.key === 's' && !e.ctrlKey && !e.metaKey) {
      if (selectedAliasObj && isActive($sessions[selectedAliasObj.name])) {
        e.preventDefault();
        stop(selectedAliasObj);
      }
      return;
    }
    if (e.key === 'S' && e.shiftKey) {
      if (runningCount > 0) {
        e.preventDefault();
        confirmStopAll = true;
      }
      return;
    }
    if (e.key === 'r' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      refresh();
    }
  }
</script>

<div class="flex h-full flex-col">
  <div class="flex items-center justify-between gap-3 border-b border-border bg-background px-4 py-2">
    <div class="flex items-center gap-2 text-xs text-muted-foreground">
      {#if $aliasesPath}
        <span class="font-mono">
          {$aliasesPath.split(/[\\/]/).pop()}
        </span>
        <span>·</span>
      {/if}
      <span>{$aliases.length} aliases</span>
      {#if runningCount > 0}
        <span class="text-status-ok">· {runningCount} active</span>
      {/if}
    </div>
    <div class="flex items-center gap-1.5">
      <span class="hidden text-[10px] text-muted-foreground sm:inline">
        <Kbd>↑↓</Kbd> nav · <Kbd>Enter</Kbd> start · <Kbd>s</Kbd> stop · <Kbd>/</Kbd> search
      </span>
      {#if runningCount > 0}
        <Button variant="outline" size="sm" onclick={() => (confirmStopAll = true)}>
          <PowerOff class="h-3.5 w-3.5" /> Stop all
        </Button>
      {/if}
      <Button variant="outline" size="sm" onclick={refresh} disabled={$loading.aliases}>
        <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.aliases ? 'animate-spin' : '')} />
        Refresh
      </Button>
    </div>
  </div>

  {#if loadError}
    <div class="border-b border-status-error/30 bg-status-error/10 px-4 py-2 text-xs text-status-error">
      {loadError}
    </div>
  {/if}

  <div class="flex min-h-0 flex-1">
    <aside class="flex w-80 shrink-0 flex-col border-r border-border bg-card/30">
      <SessionList
        {groups}
        sessions={$sessions}
        bind:selectedAlias
        bind:filter
        onSelect={selectAlias}
        onToggleGroup={toggleGroup}
        {collapsed}
        bind:searchInput
        totalCount={$aliases.length}
      />
    </aside>
    <main class="min-w-0 flex-1 bg-background">
      <SessionDetail
        alias={selectedAliasObj}
        status={selectedAlias ? $sessions[selectedAlias] : undefined}
        output={selectedOutput}
        nowTick={now}
        onStart={start}
        onStop={stop}
        onShowCredentials={(name) => (credentialsFor = name)}
        onCopyCommand={copyCommand}
      />
    </main>
  </div>
</div>

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
