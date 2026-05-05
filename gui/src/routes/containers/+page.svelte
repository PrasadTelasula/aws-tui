<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { ipc } from '$lib/ipc';
  import { clusters, loading, profile, region } from '$lib/stores/aws';
  import type { Cluster, Container, Service, Task } from '$lib/types';
  import PtyTerminal from '$lib/components/pty-terminal.svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import {
    CaretRight as ChevronRight,
    ArrowsClockwise as RefreshCw,
    Stack as Boxes,
    StackPlus as Layers,
    CircleNotch as Loader2,
    Cube as Box,
    Plug,
    Cpu,
    Database
  } from 'phosphor-svelte';

  type ServiceNode = Service & { tasks?: Task[]; expanded?: boolean; loading?: boolean };
  type ClusterNode = Cluster & { services?: ServiceNode[]; expanded?: boolean; loading?: boolean };

  let tree = $state<ClusterNode[]>([]);
  let activeTask = $state<Task | null>(null);
  let containers = $state<Container[]>([]);
  let loadingContainers = $state(false);
  let termContainer = $state<{ task: Task; container: Container } | null>(null);
  let termKey = $state(0);

  async function refresh() {
    loading.update((l) => ({ ...l, clusters: true }));
    activeTask = null;
    containers = [];
    termContainer = null;
    try {
      const list = await ipc.listClusters(get(profile), get(region));
      clusters.set(list);
      tree = list.map((c) => ({ ...c, expanded: false, services: undefined }));
    } finally {
      loading.update((l) => ({ ...l, clusters: false }));
    }
  }

  onMount(refresh);

  async function toggleCluster(c: ClusterNode) {
    c.expanded = !c.expanded;
    if (c.expanded && !c.services) {
      c.loading = true;
      tree = [...tree];
      const services = await ipc.listServices(c.name, get(profile), get(region));
      c.services = services.map((s) => ({ ...s, expanded: false }));
      c.loading = false;
    }
    tree = [...tree];
  }

  async function toggleService(c: ClusterNode, s: ServiceNode) {
    s.expanded = !s.expanded;
    if (s.expanded && !s.tasks) {
      s.loading = true;
      tree = [...tree];
      s.tasks = await ipc.listTasks(c.name, s.name, get(profile), get(region));
      s.loading = false;
    }
    tree = [...tree];
  }

  async function selectTask(t: Task) {
    if (activeTask?.arn === t.arn) return;
    activeTask = t;
    termContainer = null;
    loadingContainers = true;
    containers = [];
    try {
      containers = await ipc.listContainers(t.arn, t.cluster, get(profile), get(region));
    } finally {
      loadingContainers = false;
    }
  }

  function execContainer(task: Task, container: Container) {
    termContainer = { task, container };
    termKey += 1;
  }

  function shortId(arn: string): string {
    return arn.split('/').pop() ?? arn;
  }

  function serviceHealth(s: Service): 'ok' | 'warn' | 'muted' {
    if (s.running === s.desired && s.desired > 0) return 'ok';
    if (s.running < s.desired) return 'warn';
    return 'muted';
  }

  let totalRunning = $derived($clusters.reduce((a, c) => a + c.runningTasks, 0));
</script>

<div class="tui-screen">
  <!-- Toolbar -->
  <div class="tui-toolbar">
    <div class="tui-toolbar-title">
      <span class="tui-toolbar-title-icon"><Boxes size={15} weight="regular" /></span>
      ECS Clusters
    </div>
    <div class="tui-toolbar-stats">
      <span class="tui-stat">
        <strong>{$clusters.length}</strong> cluster{$clusters.length !== 1 ? 's' : ''}
      </span>
      {#if totalRunning > 0}
        <span class="tui-stat tui-stat-ok">
          <StatusDot tone="ok" size={5} />
          <strong>{totalRunning}</strong> running tasks
        </span>
      {/if}
    </div>
    <div class="tui-toolbar-spacer"></div>
    <button
      type="button"
      class="tui-btn tui-btn-ghost tui-btn-sm"
      onclick={refresh}
      disabled={$loading.clusters}
    >
      <RefreshCw size={12} weight="regular" class={$loading.clusters ? 'tui-spinner' : ''} />
      Refresh
    </button>
  </div>

  <div class="tui-split tui-split-narrow">
    <!-- Tree sidebar -->
    <div class="tui-split-list">
      <div class="tui-split-list-header" style="padding-bottom: 8px;">
        <div class="tui-section-label" style="padding: 0;">
          <span class="tui-section-label-text">
            <Layers size={11} weight="bold" />
            Cluster Tree
          </span>
        </div>
      </div>

      <div class="tui-split-list-body">
        {#if $loading.clusters}
          <div class="tui-empty">
            <Loader2 class="tui-spinner" size={20} />
            <div class="tui-empty-sub">Loading clusters…</div>
          </div>
        {:else if tree.length === 0}
          <div class="tui-empty">
            <div class="tui-empty-icon"><Boxes size={22} weight="thin" /></div>
            <div class="tui-empty-sub">No clusters found</div>
          </div>
        {/if}

        {#each tree as c (c.arn)}
          <button
            type="button"
            class="tui-tree-row is-cluster"
            onclick={() => toggleCluster(c)}
          >
            <span class="tui-tree-chev" class:is-open={c.expanded}>
              <ChevronRight size={11} weight="bold" />
            </span>
            <span class="tui-tree-icon-cluster"><Layers size={13} weight="regular" /></span>
            <span class="tui-tree-name tui-tree-cluster-name">{c.name}</span>
            <span class={`tui-tree-count ${c.runningTasks > 0 ? 'is-ok' : ''}`}>
              {c.runningTasks}
            </span>
          </button>

          {#if c.expanded}
            {#if c.loading}
              <div class="tui-tree-row is-service" style="color: var(--tui-fg-4); cursor: default;">
                <Loader2 class="tui-spinner" size={11} />
                <span>Loading…</span>
              </div>
            {/if}
            {#each c.services ?? [] as s (s.arn)}
              {@const health = serviceHealth(s)}
              <button
                type="button"
                class="tui-tree-row is-service"
                onclick={() => toggleService(c, s)}
              >
                <span class="tui-tree-chev" class:is-open={s.expanded}>
                  <ChevronRight size={10} weight="bold" />
                </span>
                <span class="tui-tree-name">{s.name}</span>
                <span class={`tui-tree-count is-${health}`}>{s.running}/{s.desired}</span>
              </button>

              {#if s.expanded}
                {#if s.loading}
                  <div class="tui-tree-row is-task" style="color: var(--tui-fg-4); cursor: default;">
                    <Loader2 class="tui-spinner" size={10} />
                    <span>Loading…</span>
                  </div>
                {/if}
                {#each s.tasks ?? [] as t (t.arn)}
                  {@const isActive = activeTask?.arn === t.arn}
                  <button
                    type="button"
                    class="tui-tree-row is-task"
                    class:is-active={isActive}
                    onclick={() => selectTask(t)}
                  >
                    <StatusDot
                      tone={t.lastStatus === 'RUNNING' ? 'ok' : 'muted'}
                      size={6}
                    />
                    <span class="tui-tree-name">{shortId(t.arn).slice(0, 12)}…</span>
                    {#if isActive}
                      <span style="color: var(--tui-accent-2);">●</span>
                    {/if}
                  </button>
                {/each}
              {/if}
            {/each}
          {/if}
        {/each}
      </div>
    </div>

    <!-- Right: task detail + containers + terminal -->
    <div class="tui-split-detail">
      {#if activeTask}
        {@const task = activeTask}
        <div class="tui-inst-detail">
          <div class="tui-inst-hero">
            <div class="tui-inst-hero-info">
              <h1 class="tui-inst-hero-title" style="font-family: var(--tui-font-mono); font-size: 18px;">
                {shortId(task.arn)}
              </h1>
              <div class="tui-inst-hero-id">
                {task.cluster} · {task.launchType}
              </div>
            </div>
            <span class={`tui-pill tui-pill-${task.lastStatus === 'RUNNING' ? 'ok' : 'muted'} tui-pill-md`}>
              <StatusDot
                tone={task.lastStatus === 'RUNNING' ? 'ok' : 'muted'}
                size={6}
              />
              {task.lastStatus}
            </span>
          </div>

          <div class="tui-inst-section">
            <div class="tui-section-label">
              <span class="tui-section-label-text">
                <Box size={12} weight="bold" />
                Containers
                <span class="tui-section-count">{containers.length}</span>
              </span>
            </div>

            {#if loadingContainers}
              <div style="display: flex; align-items: center; gap: 10px; color: var(--tui-fg-3); font-size: 12px; padding: 8px 0;">
                <Loader2 class="tui-spinner" size={14} />
                Loading containers…
              </div>
            {:else if containers.length === 0}
              <p style="color: var(--tui-fg-4); font-size: 12px;">No containers found.</p>
            {:else}
              <div class="tui-container-stack">
                {#each containers as co (co.name)}
                  {@const isExec = termContainer?.container.name === co.name}
                  {@const running = co.lastStatus === 'RUNNING'}
                  <div class="tui-container-card" class:is-active={isExec}>
                    <div class="tui-container-card-icon" class:is-running={running}>
                      <Box size={18} weight="regular" />
                    </div>
                    <div class="tui-container-card-info">
                      <div class="tui-container-card-row">
                        <span class="tui-container-card-name">{co.name}</span>
                        <span class={`tui-pill tui-pill-${running ? 'ok' : 'muted'} tui-pill-sm`}>
                          <StatusDot tone={running ? 'ok' : 'muted'} size={5} />
                          {co.lastStatus}
                        </span>
                        {#if co.health}
                          <span class={`tui-pill tui-pill-${co.health === 'HEALTHY' ? 'ok' : 'warn'} tui-pill-sm`}>
                            {co.health}
                          </span>
                        {/if}
                      </div>
                      <div class="tui-container-card-image">{co.image || 'No image'}</div>
                    </div>
                    {#if running}
                      <button
                        type="button"
                        class={`tui-btn tui-btn-${isExec ? 'outline' : 'default'} tui-btn-sm`}
                        onclick={() => execContainer(task, co)}
                      >
                        <Plug size={12} weight="regular" />
                        {isExec ? 'Active' : 'Exec'}
                      </button>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>

        {#if termContainer}
          {@const { task: t, container } = termContainer}
          {@const ptyId = `ecs-${shortId(t.arn)}-${container.name}-${termKey}`}
          <div class="tui-pty-footer">
            <PtyTerminal
              {ptyId}
              title="exec · {container.name} · {shortId(t.arn)}"
              onReady={async (rows, cols) => {
                await ipc.ptyOpenEcsExec(
                  ptyId,
                  t.cluster,
                  t.arn,
                  container.name,
                  undefined,
                  get(profile),
                  get(region),
                  rows,
                  cols
                );
              }}
              onClose={() => (termContainer = null)}
            />
          </div>
        {/if}
      {:else}
        <div class="tui-empty">
          <div class="tui-empty-icon"><Boxes size={22} weight="thin" /></div>
          <div class="tui-empty-title">Select a task</div>
          <div class="tui-empty-sub">Expand a cluster → service → task in the tree on the left.</div>
        </div>
      {/if}
    </div>
  </div>
</div>
