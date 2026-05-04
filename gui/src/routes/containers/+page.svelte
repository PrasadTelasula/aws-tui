<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { ipc } from '$lib/ipc';
  import { clusters, loading, profile, region } from '$lib/stores/aws';
  import type { Cluster, Container, Service, Task } from '$lib/types';
  import PtyTerminal from '$lib/components/pty-terminal.svelte';
  import { Badge, Button } from '$lib/components/ui';
  import {
    ChevronRight,
    ChevronDown,
    RefreshCw,
    Terminal,
    Boxes,
    Layers,
    CircleDot,
    Loader2,
    Box,
    PlugZap,
    Hash
  } from 'lucide-svelte';
  import { cn } from '$lib/utils';

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

<div class="flex h-full flex-col">
  <!-- Toolbar -->
  <div class="flex h-11 shrink-0 items-center gap-3 border-b border-border bg-card/30 px-4">
    <Boxes class="h-4 w-4 text-muted-foreground/60" />
    <span class="text-sm font-semibold">Containers</span>
    {#if $clusters.length > 0}
      <div class="flex items-center gap-1.5">
        <span class="rounded-full bg-muted px-2 py-0.5 text-[11px] tabular-nums text-muted-foreground">
          {$clusters.length} cluster{$clusters.length !== 1 ? 's' : ''}
        </span>
        {#if totalRunning > 0}
          <span class="rounded-full bg-status-ok/15 px-2 py-0.5 text-[11px] tabular-nums text-status-ok">
            {totalRunning} running
          </span>
        {/if}
      </div>
    {/if}
    <Button
      variant="ghost"
      size="sm"
      onclick={refresh}
      disabled={$loading.clusters}
      class="ml-auto h-8 text-muted-foreground hover:text-foreground"
    >
      <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.clusters ? 'animate-spin' : '')} />
      Refresh
    </Button>
  </div>

  <!-- Main split -->
  <div class="flex min-h-0 flex-1">
    <!-- Tree sidebar -->
    <div class="flex w-60 shrink-0 flex-col border-r border-border bg-sidebar-background">
      <div class="border-b border-border/60 px-4 py-2.5">
        <p class="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/50">
          Clusters
        </p>
      </div>

      <div class="min-h-0 flex-1 overflow-auto py-1.5">
        {#if $loading.clusters}
          <div class="flex items-center gap-2 px-4 py-5 text-xs text-muted-foreground/60">
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            Loading clusters…
          </div>
        {:else if tree.length === 0}
          <div class="flex flex-col items-center justify-center gap-2 py-10 text-center">
            <Boxes class="h-7 w-7 text-muted-foreground/20" />
            <p class="text-xs text-muted-foreground/50">No clusters found</p>
          </div>
        {/if}

        {#each tree as c (c.arn)}
          <!-- Cluster header -->
          <button
            class="flex w-full items-center gap-2.5 px-3 py-2 text-left transition-colors hover:bg-sidebar-accent/60"
            onclick={() => toggleCluster(c)}
          >
            <span class="text-muted-foreground/50">
              {#if c.expanded}
                <ChevronDown class="h-3.5 w-3.5" />
              {:else}
                <ChevronRight class="h-3.5 w-3.5" />
              {/if}
            </span>
            <Layers class="h-3.5 w-3.5 shrink-0 text-blue-400" />
            <span class="flex-1 truncate font-mono text-[11px] font-semibold">{c.name}</span>
            <span class={cn(
              'shrink-0 rounded-full px-1.5 py-0.5 text-[10px] tabular-nums',
              c.runningTasks > 0
                ? 'bg-status-ok/15 text-status-ok'
                : 'bg-muted/60 text-muted-foreground'
            )}>
              {c.runningTasks}
            </span>
          </button>

          {#if c.expanded}
            <!-- Services -->
            <div class="ml-4 border-l border-border/40 pb-1">
              {#if c.loading}
                <div class="flex items-center gap-2 py-2 pl-3 text-[11px] text-muted-foreground/60">
                  <Loader2 class="h-3 w-3 animate-spin" /> Loading…
                </div>
              {/if}
              {#each c.services ?? [] as s (s.arn)}
                {@const health = serviceHealth(s)}
                <button
                  class="flex w-full items-center gap-2 px-3 py-1.5 text-left transition-colors hover:bg-sidebar-accent/50"
                  onclick={() => toggleService(c, s)}
                >
                  <span class="text-muted-foreground/40">
                    {#if s.expanded}
                      <ChevronDown class="h-3 w-3" />
                    {:else}
                      <ChevronRight class="h-3 w-3" />
                    {/if}
                  </span>
                  <span class="flex-1 truncate font-mono text-[11px] text-foreground/80">{s.name}</span>
                  <!-- Health indicator -->
                  <span class={cn(
                    'shrink-0 rounded px-1.5 py-0.5 text-[10px] tabular-nums font-medium',
                    health === 'ok' ? 'bg-status-ok/15 text-status-ok' :
                    health === 'warn' ? 'bg-status-warn/15 text-status-warn' :
                    'bg-muted/60 text-muted-foreground'
                  )}>
                    {s.running}/{s.desired}
                  </span>
                </button>

                {#if s.expanded}
                  <!-- Tasks -->
                  <div class="ml-4 border-l border-border/30 pb-0.5">
                    {#if s.loading}
                      <div class="flex items-center gap-2 py-1.5 pl-3 text-[11px] text-muted-foreground/60">
                        <Loader2 class="h-3 w-3 animate-spin" /> Loading…
                      </div>
                    {/if}
                    {#each s.tasks ?? [] as t (t.arn)}
                      {@const isActive = activeTask?.arn === t.arn}
                      <button
                        class={cn(
                          'group flex w-full items-center gap-2 py-1.5 pl-3 pr-2 text-left transition-colors',
                          isActive
                            ? 'bg-primary/10 text-foreground'
                            : 'hover:bg-sidebar-accent/40 text-foreground/70'
                        )}
                        onclick={() => selectTask(t)}
                      >
                        <span class={cn(
                          'h-1.5 w-1.5 shrink-0 rounded-full',
                          t.lastStatus === 'RUNNING'
                            ? 'bg-status-ok'
                            : 'bg-muted-foreground/30'
                        )}></span>
                        <Hash class="h-2.5 w-2.5 shrink-0 text-muted-foreground/30" />
                        <span class="truncate font-mono text-[10px]">
                          {shortId(t.arn).slice(0, 8)}…
                        </span>
                        {#if isActive}
                          <span class="ml-auto h-1.5 w-1.5 shrink-0 rounded-full bg-primary"></span>
                        {/if}
                      </button>
                    {/each}
                  </div>
                {/if}
              {/each}
            </div>
          {/if}
        {/each}
      </div>
    </div>

    <!-- Right: task detail + containers + terminal -->
    <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
      {#if activeTask}
        {@const task = activeTask}
        <div class="min-h-0 flex-1 overflow-auto">
          <!-- Task hero -->
          <div class="border-b border-border/60 px-8 py-6">
            <div class="flex items-start justify-between gap-4">
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class={cn(
                    'h-2.5 w-2.5 shrink-0 rounded-full',
                    task.lastStatus === 'RUNNING'
                      ? 'bg-status-ok shadow-[0_0_8px_theme(colors.status.ok/50%)]'
                      : 'bg-muted-foreground/30'
                  )}></span>
                  <h1 class="truncate font-mono text-lg font-bold tracking-tight">
                    {shortId(task.arn)}
                  </h1>
                </div>
                <p class="mt-1.5 text-xs text-muted-foreground/60">
                  {task.launchType} · {task.cluster}
                </p>
              </div>
              <span class={cn(
                'shrink-0 inline-flex items-center rounded-full border px-3 py-1 text-xs font-semibold',
                task.lastStatus === 'RUNNING'
                  ? 'border-status-ok/20 bg-status-ok/12 text-status-ok'
                  : 'border-border bg-muted/50 text-muted-foreground'
              )}>
                {task.lastStatus}
              </span>
            </div>
          </div>

          <!-- Containers -->
          <div class="px-8 py-6">
            <h3 class="mb-4 text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/50">
              Containers ({containers.length})
            </h3>

            {#if loadingContainers}
              <div class="flex items-center gap-2.5 text-sm text-muted-foreground/60">
                <Loader2 class="h-4 w-4 animate-spin" />
                Loading containers…
              </div>
            {:else if containers.length === 0}
              <p class="text-sm text-muted-foreground/50">No containers found.</p>
            {:else}
              <div class="space-y-3">
                {#each containers as c (c.name)}
                  {@const isExec = termContainer?.container.name === c.name}
                  <div class={cn(
                    'rounded-xl border bg-card transition-all',
                    isExec
                      ? 'border-primary/30 shadow-[0_0_0_1px_theme(colors.primary/20%)]'
                      : 'border-border/60 hover:border-border'
                  )}>
                    <div class="flex items-center gap-4 p-4">
                      <!-- Container icon -->
                      <div class={cn(
                        'flex h-10 w-10 shrink-0 items-center justify-center rounded-lg',
                        c.lastStatus === 'RUNNING'
                          ? 'bg-status-ok/10'
                          : 'bg-muted/50'
                      )}>
                        <Box class={cn(
                          'h-5 w-5',
                          c.lastStatus === 'RUNNING'
                            ? 'text-status-ok'
                            : 'text-muted-foreground/30'
                        )} />
                      </div>

                      <!-- Info -->
                      <div class="min-w-0 flex-1">
                        <div class="flex items-center gap-2">
                          <span class="font-mono text-sm font-semibold">{c.name}</span>
                          <span class={cn(
                            'inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-medium',
                            c.lastStatus === 'RUNNING'
                              ? 'border-status-ok/20 bg-status-ok/10 text-status-ok'
                              : 'border-border bg-muted/50 text-muted-foreground'
                          )}>
                            <span class={cn(
                              'h-1 w-1 rounded-full',
                              c.lastStatus === 'RUNNING' ? 'bg-status-ok' : 'bg-muted-foreground/40'
                            )}></span>
                            {c.lastStatus}
                          </span>
                          {#if c.health}
                            <span class={cn(
                              'rounded-full border px-2 py-0.5 text-[10px] font-medium',
                              c.health === 'HEALTHY'
                                ? 'border-status-ok/20 bg-status-ok/10 text-status-ok'
                                : 'border-status-warn/20 bg-status-warn/10 text-status-warn'
                            )}>
                              {c.health}
                            </span>
                          {/if}
                        </div>
                        <p class="mt-1 truncate font-mono text-[11px] text-muted-foreground/60">
                          {c.image || 'No image'}
                        </p>
                      </div>

                      <!-- Exec button -->
                      {#if c.lastStatus === 'RUNNING'}
                        <Button
                          size="sm"
                          variant={isExec ? 'secondary' : 'outline'}
                          onclick={() => execContainer(task, c)}
                          class="shrink-0 gap-1.5"
                        >
                          <PlugZap class="h-3.5 w-3.5" />
                          {isExec ? 'Active' : 'Exec'}
                        </Button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>

        <!-- ECS exec terminal -->
        {#if termContainer}
          {@const { task: t, container } = termContainer}
          {@const ptyId = `ecs-${shortId(t.arn)}-${container.name}-${termKey}`}
          <div class="h-72 shrink-0 border-t border-border">
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
        <!-- Empty state -->
        <div class="flex flex-1 flex-col items-center justify-center gap-4 text-center">
          <div class="flex h-16 w-16 items-center justify-center rounded-2xl border border-border/60 bg-card">
            <Boxes class="h-7 w-7 text-muted-foreground/30" />
          </div>
          <div>
            <p class="text-sm font-semibold text-foreground/70">Select a task</p>
            <p class="mt-1 text-xs text-muted-foreground/50">
              Expand a cluster → service → task in the sidebar
            </p>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
