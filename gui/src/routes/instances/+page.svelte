<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { ipc } from '$lib/ipc';
  import { instances, loading, profile, region } from '$lib/stores/aws';
  import type { Instance } from '$lib/types';
  import PtyTerminal from '$lib/components/pty-terminal.svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import {
    ArrowsClockwise as RefreshCw,
    MagnifyingGlass as Search,
    HardDrives as Server,
    Copy,
    MapPin,
    Cpu,
    TreeStructure as NetIcon,
    Globe,
    Tag,
    CircleNotch as Loader2,
    Plug,
    X,
    CaretDown,
    CaretUp
  } from 'phosphor-svelte';

  let filter = $state('');
  let selected = $state<Instance | null>(null);
  let copiedIp = $state<string | null>(null);
  /** When at least one SSM tab is open, the info panel collapses to a
   *  one-row hero strip. The user can expand it to see Network + Tags. */
  let detailsExpanded = $state(false);

  /** Open SSM tabs — multiple instances can be connected at once. The
   *  PTYs stay mounted in a stack; only the active one is visible. */
  type SsmTab = { id: string; instance: Instance };
  let terms = $state<SsmTab[]>([]);
  let activeTermId = $state<string | null>(null);

  async function refresh() {
    loading.update((l) => ({ ...l, instances: true }));
    selected = null;
    // Don't drop active terminals on refresh — they're independent of the
    // instance list. The user can close each tab manually.
    try {
      instances.set(await ipc.listInstances(get(profile), get(region)));
    } finally {
      loading.update((l) => ({ ...l, instances: false }));
    }
  }

  onMount(refresh);

  function connectSsm(inst: Instance) {
    // If a tab for this instance already exists, just switch to it.
    const existing = terms.find((t) => t.instance.id === inst.id);
    if (existing) {
      activeTermId = existing.id;
      return;
    }
    const id = `ssm-${inst.id}-${Date.now()}`;
    terms = [...terms, { id, instance: inst }];
    activeTermId = id;
  }

  function closeTerm(id: string) {
    const idx = terms.findIndex((t) => t.id === id);
    if (idx === -1) return;
    const next = terms.filter((t) => t.id !== id);
    terms = next;
    if (activeTermId === id) {
      // Prefer the tab to the right; fall back to the left; null when empty.
      activeTermId = next[idx]?.id ?? next[idx - 1]?.id ?? next[0]?.id ?? null;
    }
  }

  function ec2StateTone(state: string): 'ok' | 'warn' | 'error' | 'muted' {
    if (state === 'running') return 'ok';
    if (state === 'pending' || state === 'stopping') return 'warn';
    if (state === 'terminated' || state === 'shutting-down') return 'error';
    return 'muted';
  }

  async function copyIp(ip: string) {
    try {
      const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
      await writeText(ip);
    } catch {
      try { await navigator.clipboard.writeText(ip); } catch { return; }
    }
    copiedIp = ip;
    setTimeout(() => { if (copiedIp === ip) copiedIp = null; }, 1500);
  }

  let filtered = $derived.by(() => {
    const f = filter.trim().toLowerCase();
    return f
      ? $instances.filter((i) =>
          [i.id, i.name, i.state, i.instanceType, i.privateIp, i.az]
            .filter(Boolean)
            .some((v) => v!.toLowerCase().includes(f))
        )
      : $instances;
  });

  let runningCount = $derived($instances.filter((i) => i.state === 'running').length);
</script>

<div class="tui-screen">
  <!-- Toolbar -->
  <div class="tui-toolbar">
    <div class="tui-toolbar-title">
      <span class="tui-toolbar-title-icon"><Server size={15} weight="regular" /></span>
      EC2 Instances
    </div>
    <div class="tui-toolbar-stats">
      <span class="tui-stat"><strong>{$instances.length}</strong> total</span>
      {#if runningCount > 0}
        <span class="tui-stat tui-stat-ok">
          <StatusDot tone="ok" size={5} />
          <strong>{runningCount}</strong> running
        </span>
      {/if}
    </div>
    <div class="tui-toolbar-spacer"></div>
    <button
      type="button"
      class="tui-btn tui-btn-ghost tui-btn-sm"
      onclick={refresh}
      disabled={$loading.instances}
    >
      <RefreshCw size={12} weight="regular" class={$loading.instances ? 'tui-spinner' : ''} />
      Refresh
    </button>
  </div>

  <div class="tui-split">
    <!-- LEFT: instance list -->
    <div class="tui-split-list">
      <div class="tui-split-list-header">
        <div class="tui-search">
          <span class="tui-search-icon"><Search size={13} weight="regular" /></span>
          <input
            class="tui-search-input"
            placeholder="Search instances…"
            bind:value={filter}
            spellcheck={false}
          />
        </div>
        <div class="tui-split-list-meta">
          <span>{filtered.length} results</span>
          {#if $instances[0]?.az}
            <span class="tui-split-list-meta-mono">
              {$instances[0].az.replace(/[a-z]$/, '')}
            </span>
          {/if}
        </div>
      </div>

      <div class="tui-split-list-body" style="padding-bottom: 0;">
        {#if $loading.instances}
          <div class="tui-empty">
            <Loader2 class="tui-spinner" size={20} />
            <div class="tui-empty-sub">Loading instances…</div>
          </div>
        {:else if filtered.length === 0}
          <div class="tui-empty">
            <div class="tui-empty-icon"><Server size={22} weight="thin" /></div>
            <div class="tui-empty-title">{filter ? 'No instances match' : 'No instances found'}</div>
          </div>
        {:else}
          {#each filtered as inst (inst.id)}
            {@const tone = ec2StateTone(inst.state)}
            {@const isSelected = selected?.id === inst.id}
            <button
              type="button"
              class="tui-inst-row"
              class:is-selected={isSelected}
              onclick={() => (selected = inst)}
            >
              <StatusDot tone={tone} size={8} pulse={inst.state === 'pending' || inst.state === 'stopping'} />
              <div class="tui-inst-row-info">
                <div class="tui-inst-row-name">{inst.name ?? inst.id}</div>
                <div class="tui-inst-row-id">{inst.id}</div>
                <div class="tui-inst-row-meta">
                  <span class="tui-inst-row-type">{inst.instanceType}</span>
                  {#if inst.az}
                    <span>{inst.az}</span>
                  {/if}
                </div>
              </div>
              <span class={`tui-pill tui-pill-${tone} tui-pill-sm`}>{inst.state}</span>
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <!-- RIGHT: detail pane -->
    <div class="tui-split-detail">
      {#if selected}
        {@const inst = selected}
        {@const tone = ec2StateTone(inst.state)}
        <div class="tui-inst-detail" class:is-compact={terms.length > 0} class:is-expanded={detailsExpanded}>
          <div class="tui-inst-hero">
            <div class="tui-inst-hero-info">
              <h1 class="tui-inst-hero-title">{inst.name ?? inst.id}</h1>
              <div class="tui-inst-hero-id">{inst.id}</div>
              <div class="tui-chip-row">
                <span class="tui-chip">
                  <span class="tui-chip-icon"><Cpu size={11} /></span>
                  <span class="tui-chip-mono">{inst.instanceType}</span>
                </span>
                {#if inst.az}
                  <span class="tui-chip">
                    <span class="tui-chip-icon"><MapPin size={11} /></span>
                    {inst.az}
                  </span>
                {/if}
                {#if inst.vpcId}
                  <span class="tui-chip">
                    <span class="tui-chip-icon"><NetIcon size={11} /></span>
                    <span class="tui-chip-mono">{inst.vpcId}</span>
                  </span>
                {/if}
              </div>
            </div>
            <div class="tui-inst-hero-actions">
              <span class={`tui-pill tui-pill-${tone} tui-pill-md`}>
                <StatusDot tone={tone} size={6} />
                {inst.state}
              </span>
              {#if inst.state === 'running'}
                {@const alreadyOpen = terms.some((t) => t.instance.id === inst.id)}
                <button
                  type="button"
                  class={`tui-btn ${alreadyOpen ? 'tui-btn-outline' : 'tui-btn-default'} tui-btn-md`}
                  onclick={() => connectSsm(inst)}
                  title={alreadyOpen ? 'Switch to existing SSM tab' : 'Open a new SSM session'}
                >
                  <Plug size={14} weight="regular" />
                  {alreadyOpen ? 'Switch to SSM' : 'Connect via SSM'}
                </button>
              {/if}
              {#if terms.length > 0}
                <button
                  type="button"
                  class="tui-btn tui-btn-ghost tui-btn-sm"
                  onclick={() => (detailsExpanded = !detailsExpanded)}
                  title={detailsExpanded ? 'Hide network and tags' : 'Show network and tags'}
                >
                  {#if detailsExpanded}
                    <CaretUp size={11} weight="bold" />
                    Hide
                  {:else}
                    <CaretDown size={11} weight="bold" />
                    Details
                  {/if}
                </button>
              {/if}
            </div>
          </div>

          {#if inst.privateIp || inst.publicIp}
            <div class="tui-inst-section">
              <div class="tui-section-label">
                <span class="tui-section-label-text">
                  <NetIcon size={12} weight="bold" />
                  Network
                </span>
              </div>
              <div class="tui-card-grid">
                {#if inst.privateIp}
                  <button
                    type="button"
                    class="tui-info-card"
                    onclick={() => copyIp(inst.privateIp!)}
                  >
                    <span class="tui-info-card-label">
                      <NetIcon size={11} weight="bold" />
                      Private IP
                    </span>
                    <span class="tui-info-card-value">
                      {copiedIp === inst.privateIp ? '✓ Copied' : inst.privateIp}
                    </span>
                    <span class="tui-info-card-copy"><Copy size={12} /></span>
                  </button>
                {/if}
                {#if inst.publicIp}
                  <button
                    type="button"
                    class="tui-info-card"
                    onclick={() => copyIp(inst.publicIp!)}
                  >
                    <span class="tui-info-card-label">
                      <Globe size={11} weight="bold" />
                      Public IP
                    </span>
                    <span class="tui-info-card-value">
                      {copiedIp === inst.publicIp ? '✓ Copied' : inst.publicIp}
                    </span>
                    <span class="tui-info-card-copy"><Copy size={12} /></span>
                  </button>
                {:else}
                  <div class="tui-info-card" style="cursor: default;">
                    <span class="tui-info-card-label">
                      <Globe size={11} weight="bold" />
                      Public IP
                    </span>
                    <span class="tui-info-card-value is-muted">none</span>
                  </div>
                {/if}
              </div>
            </div>
          {/if}

          {#if Object.keys(inst.tags).length > 0}
            <div class="tui-inst-section">
              <div class="tui-section-label">
                <span class="tui-section-label-text">
                  <Tag size={12} weight="bold" />
                  Tags
                  <span class="tui-section-count">{Object.keys(inst.tags).length}</span>
                </span>
              </div>
              <div class="tui-tags">
                {#each Object.entries(inst.tags) as [k, v] (k)}
                  <span class="tui-tag-pill">
                    <span class="tui-tag-key">{k}</span>
                    <span class="tui-tag-val">{v}</span>
                  </span>
                {/each}
              </div>
            </div>
          {/if}
        </div>

        {#if terms.length > 0}
          <div class="tui-pty-footer is-fullspace">
            <!-- Tab strip — one entry per open SSM session -->
            <div class="tui-pty-tabs" role="tablist">
              {#each terms as t (t.id)}
                <div
                  role="tab"
                  aria-selected={t.id === activeTermId}
                  class="tui-pty-tab"
                  class:is-active={t.id === activeTermId}
                  onclick={() => (activeTermId = t.id)}
                  onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); activeTermId = t.id; } }}
                  tabindex="0"
                >
                  <Plug size={11} weight={t.id === activeTermId ? 'bold' : 'regular'} />
                  <span class="tui-pty-tab-label">{t.instance.name ?? t.instance.id}</span>
                  <button
                    type="button"
                    class="tui-pty-tab-close"
                    title="Close terminal"
                    onclick={(e) => { e.stopPropagation(); closeTerm(t.id); }}
                  >
                    <X size={10} weight="bold" />
                  </button>
                </div>
              {/each}
            </div>
            <!-- Stacked PTYs — only the active one is visible, but every
                 PTY stays mounted so its session/scrollback persists. -->
            <div class="tui-pty-stack">
              {#each terms as t (t.id)}
                {@const tinst = t.instance}
                <div class="tui-pty-stack-item" class:is-visible={t.id === activeTermId} aria-hidden={t.id !== activeTermId}>
                  <PtyTerminal
                    ptyId={t.id}
                    title="SSM · {tinst.name ?? tinst.id} · {tinst.id}"
                    onReady={async (rows, cols) => {
                      await ipc.ptyOpenSsm(t.id, tinst.id, get(profile), get(region), rows, cols);
                    }}
                    onClose={() => closeTerm(t.id)}
                  />
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {:else}
        <div class="tui-empty">
          <div class="tui-empty-icon"><Server size={22} weight="thin" /></div>
          <div class="tui-empty-title">Select an instance</div>
          <div class="tui-empty-sub">Choose from the list to view details and connect.</div>
        </div>
      {/if}
    </div>
  </div>
</div>
