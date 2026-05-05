<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { ipc } from '$lib/ipc';
  import type { Alias, SessionStatus } from '$lib/types';
  import { aliases, aliasesPath, sessions, loading } from '$lib/stores/aws';
  import { groupAliases, isActive, kindLabel, kindIcon } from '$lib/sessions-helpers';
  import type { AliasGroup } from '$lib/sessions-helpers';
  import { flatten } from '$lib/components/sessions/session-list.svelte';
  import SessionDetail from '$lib/components/sessions/session-detail.svelte';
  import CredentialsModal from '$lib/components/credentials-modal.svelte';
  import ConfirmModal from '$lib/components/confirm-modal.svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import {
    Power as PowerOff,
    ArrowsClockwise as RefreshCw,
    Pulse as Activity,
    Tag,
    SignIn as LogIn,
    Shield,
    TreeStructure as Network,
    MagnifyingGlass as SearchIcon,
    CaretDown as ChevronDown
  } from 'phosphor-svelte';
  import { uptimeFrom } from '$lib/utils';
  import { stateTone, portHint } from '$lib/sessions-helpers';

  function kindMeta(kind: Alias['kind']): { tone: string; Icon: any; label: string } {
    switch (kind) {
      case 'sso-login':   return { tone: 'violet', Icon: LogIn,   label: 'SSO' };
      case 'iam-profile': return { tone: 'cyan',   Icon: Shield,  label: 'IAM' };
      case 'ssm-session': return { tone: 'amber',  Icon: Network, label: 'SSM' };
      default:            return { tone: 'muted',  Icon: Tag,     label: 'OTH' };
    }
  }

  function stateBadge(state: SessionStatus['state'] | undefined): { cls: string; text: string } | null {
    if (!state) return null;
    if (state === 'running' || state === 'connected') return { cls: 'is-ok',   text: 'active' };
    if (state === 'starting')                          return { cls: 'is-info', text: 'starting' };
    if (state === 'expired')                           return { cls: 'is-warn', text: 'expired' };
    if (state === 'error')                             return { cls: 'is-err',  text: 'error' };
    return null;
  }

  function aliasSubline(a: Alias, st: SessionStatus | undefined, nowMs: number): string {
    if (st && isActive(st) && st.startedAt) {
      void nowMs;
      return `${a.command.split(/\s+/)[0] ?? a.kind} · up ${uptimeFrom(st.startedAt)}`;
    }
    if (a.profile) return `${a.kind} · ${a.profile}`;
    const port = portHint(a);
    return port ?? a.command;
  }

  let filter = $state('');
  let loadError = $state<string | null>(null);
  let selectedAlias = $state<string | null>(null);
  let selectedOutput = $state<string[]>([]);
  let credentialsFor = $state<string | null>(null);
  let confirmStopAll = $state(false);
  let collapsed = $state<Record<string, boolean>>({});
  let now = $state(Date.now());
  let searchInput: HTMLInputElement | undefined = $state();
  let groupBy = $state<'group' | 'status' | 'flat'>('group');

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
        /* ignore */
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

  let filteredAliases = $derived($aliases.filter(matchesFilter));

  let groups: AliasGroup[] = $derived.by(() => {
    if (groupBy === 'flat') {
      return [{
        name: 'All aliases',
        icon: Tag as any,
        explicit: false,
        subgroups: [{ name: '', aliases: filteredAliases }]
      }];
    }
    if (groupBy === 'status') {
      const buckets: Record<string, Alias[]> = {
        running: [],
        starting: [],
        connected: [],
        expired: [],
        error: [],
        stopped: []
      };
      for (const a of filteredAliases) {
        const st = $sessions[a.name]?.state ?? 'stopped';
        (buckets[st] ?? buckets.stopped).push(a);
      }
      const order: Array<[string, string]> = [
        ['running',   'Active'],
        ['connected', 'Connected'],
        ['starting',  'Starting'],
        ['expired',   'Token expired'],
        ['error',     'Errors'],
        ['stopped',   'Idle']
      ];
      return order
        .filter(([k]) => (buckets[k]?.length ?? 0) > 0)
        .map(([k, label]) => ({
          name: label,
          icon: Tag as any,
          explicit: true,
          subgroups: [{ name: '', aliases: buckets[k] }]
        }));
    }
    return groupAliases(filteredAliases);
  });

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

    if (e.key === '/' && !inInput) {
      e.preventDefault();
      searchInput?.focus();
      searchInput?.select();
      return;
    }

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
    if (e.key === 'Home') {
      e.preventDefault();
      if (flat[0]?.alias) selectAlias(flat[0].alias.name);
      return;
    }
    if (e.key === 'End') {
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

<div class="tui-screen">
  <!-- Toolbar -->
  <div class="tui-toolbar">
    <div class="tui-toolbar-title">
      <span class="tui-toolbar-title-icon"><Activity size={15} weight="regular" /></span>
      Sessions
    </div>
    <div class="tui-toolbar-stats">
      <span class="tui-stat"><strong>{$aliases.length}</strong> aliases</span>
      {#if runningCount > 0}
        <span class="tui-stat tui-stat-ok">
          <StatusDot tone="ok" size={5} />
          <strong>{runningCount}</strong> active
        </span>
      {/if}
    </div>
    <div class="tui-toolbar-spacer"></div>
    <div class="tui-toolbar-shortcut-hint">
      <span><kbd class="tui-kbd">↑↓</kbd> nav</span>
      <span><kbd class="tui-kbd">↵</kbd> start</span>
      <span><kbd class="tui-kbd">s</kbd> stop</span>
      <span><kbd class="tui-kbd">/</kbd> search</span>
    </div>
    {#if runningCount > 0}
      <button
        type="button"
        class="tui-btn tui-btn-destructive tui-btn-sm"
        onclick={() => (confirmStopAll = true)}
      >
        <PowerOff size={12} weight="regular" />
        Stop all
      </button>
    {/if}
    <button
      type="button"
      class="tui-btn tui-btn-ghost tui-btn-sm"
      onclick={refresh}
      disabled={$loading.aliases}
    >
      <RefreshCw size={12} weight="regular" class={$loading.aliases ? 'tui-spinner' : ''} />
      Refresh
    </button>
  </div>

  {#if loadError}
    <div
      style="padding: 8px 14px; font-size: 12px; color: var(--tui-err); background: var(--tui-err-soft); border-bottom: 1px solid rgba(242,107,107,0.2);"
    >
      {loadError}
    </div>
  {/if}

  <div class="tui-split">
    <div class="tui-split-list">
      <div class="tui-split-list-header">
        <div class="tui-search">
          <span class="tui-search-icon">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="11" cy="11" r="7" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
            </svg>
          </span>
          <input
            bind:this={searchInput}
            bind:value={filter}
            class="tui-search-input"
            placeholder="Filter aliases…"
            spellcheck={false}
          />
          <span class="tui-search-kbd"><kbd class="tui-kbd">/</kbd></span>
        </div>
        <div class="tui-list-controls">
          <span class="tui-list-controls-label">Group by</span>
          <div class="tui-seg">
            <button
              type="button"
              class="tui-seg-btn"
              class:is-active={groupBy === 'group'}
              onclick={() => (groupBy = 'group')}
            >Type</button>
            <button
              type="button"
              class="tui-seg-btn"
              class:is-active={groupBy === 'status'}
              onclick={() => (groupBy = 'status')}
            >Status</button>
            <button
              type="button"
              class="tui-seg-btn"
              class:is-active={groupBy === 'flat'}
              onclick={() => (groupBy = 'flat')}
            >None</button>
          </div>
        </div>
        <div class="tui-split-list-meta">
          <span>{filteredAliases.length} of {$aliases.length}</span>
          <span class="tui-split-list-meta-mono">
            {runningCount > 0 ? `${runningCount} running` : 'all idle'}
          </span>
        </div>
      </div>

      <div class="tui-split-list-body">
        {#each groups as g (g.name)}
          {@const total = g.subgroups.reduce((acc, sg) => acc + sg.aliases.length, 0)}
          {@const isCollapsed = !!collapsed[g.name]}
          <button
            type="button"
            class="tui-group-header"
            class:is-collapsed={isCollapsed}
            onclick={() => toggleGroup(g.name)}
          >
            <span class="tui-group-header-chev">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 9 12 15 18 9" />
              </svg>
            </span>
            <span>{g.name}</span>
            <span class="tui-group-header-line"></span>
            <span class="tui-group-header-count">{total}</span>
          </button>
          {#if !isCollapsed}
            {#each g.subgroups as sg (sg.name || '_')}
              {#if sg.name}
                <div class="tui-subgroup">
                  <span>{sg.name}</span>
                  <span class="tui-subgroup-line"></span>
                </div>
              {/if}
              {#each sg.aliases as a (a.name)}
                {@const st = $sessions[a.name]}
                {@const selected = selectedAlias === a.name}
                {@const km = kindMeta(a.kind)}
                {@const KindIcon = km.Icon}
                {@const badge = stateBadge(st?.state)}
                {@const tone = stateTone(st?.state)}
                {@const active = isActive(st)}
                <button
                  type="button"
                  data-alias={a.name}
                  class="tui-alias-row tui-alias-row-rich"
                  class:is-selected={selected}
                  class:is-active={active}
                  onclick={() => selectAlias(a.name)}
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
                      {#if badge}
                        <span class={`tui-alias-row-state ${badge.cls}`}>{badge.text}</span>
                      {/if}
                    </span>
                    <span class="tui-alias-row-line2" title={a.command}>
                      {aliasSubline(a, st, now)}
                    </span>
                  </span>
                </button>
              {/each}
            {/each}
          {/if}
        {/each}
        {#if groups.length === 0}
          <p style="padding: 24px 16px; text-align: center; color: var(--tui-fg-4); font-size: 12px;">
            {filter ? 'No aliases match' : 'No aliases loaded'}
          </p>
        {/if}
      </div>
    </div>

    <div class="tui-split-detail">
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
    </div>
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

