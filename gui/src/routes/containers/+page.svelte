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
    Box
  } from 'lucide-svelte';
  import { cn } from '$lib/utils';

  type ServiceNode = Service & { tasks?: Task[]; expanded?: boolean; loading?: boolean };
  type ClusterNode = Cluster & { services?: ServiceNode[]; expanded?: boolean; loading?: boolean };

  let tree = $state<ClusterNode[]>([]);
  let activeTask = $state<Task | null>(null);
  let containers = $state<Container[]>([]);
  let loadingContainers = $state(false);

  // ECS exec terminal
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

  function taskShortId(arn: string): string {
    return arn.split('/').pop() ?? arn;
  }
</script>

<div class="flex h-full flex-col">
  <!-- Toolbar -->
  <div class="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card/40 px-4">
    <div class="flex items-center gap-2">
      <Boxes class="h-4 w-4 text-muted-foreground" />
      <h1 class="text-sm font-semibold">Containers</h1>
      {#if $clusters.length > 0}
        <span class="rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium tabular-nums text-muted-foreground">
          {$clusters.length} cluster{$clusters.length !== 1 ? 's' : ''}
        </span>
      {/if}
    </div>
    <Button
      variant="outline"
      size="sm"
      onclick={refresh}
      disabled={$loading.clusters}
      class="ml-auto h-8"
    >
      <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.clusters ? 'animate-spin' : '')} />
      Refresh
    </Button>
  </div>

  <!-- Main: tree sidebar + right panel -->
  <div class="flex min-h-0 flex-1">
    <!-- Tree sidebar -->
    <div class="flex w-60 shrink-0 flex-col border-r border-border bg-card/20">
      <div class="border-b border-border px-3 py-2">
        <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/60">
          Clusters
        </p>
      </div>
      <div class="min-h-0 flex-1 overflow-auto py-1">
        {#if $loading.clusters}
          <div class="flex items-center gap-2 px-3 py-5 text-xs text-muted-foreground">
            <Loader2 class="h-3.5 w-3.5 animate-spin" />
            Loading clusters…
          </div>
        {:else if tree.length === 0}
          <p class="px-3 py-8 text-center text-xs text-muted-foreground/60">No clusters found</p>
        {/if}

        {#each tree as c (c.arn)}
          <!-- Cluster row -->
          <button
            class="flex w-full items-center gap-2 px-2.5 py-2 text-left text-xs font-semibold transition-colors hover:bg-accent/40"
            onclick={() => toggleCluster(c)}
          >
            <span class="text-muted-foreground/60">
              {#if c.expanded}
                <ChevronDown class="h-3.5 w-3.5" />
              {:else}
                <ChevronRight class="h-3.5 w-3.5" />
              {/if}
            </span>
            <Layers class="h-3.5 w-3.5 shrink-0 text-blue-400/80" />
            <span class="flex-1 truncate font-mono text-[11px]">{c.name}</span>
            <span class="shrink-0 rounded-full bg-muted px-1.5 py-0.5 text-[10px] tabular-nums text-muted-foreground">
              {c.runningTasks}
            </span>
          </button>

          {#if c.expanded}
            <div class="ml-5 border-l border-border/50">
              {#if c.loading}
                <div class="flex items-center gap-2 px-3 py-2 text-[11px] text-muted-foreground">
                  <Loader2 class="h-3 w-3 animate-spin" /> Loading…
                </div>
              {/if}
              {#each c.services ?? [] as s (s.arn)}
                <!-- Service row -->
                <button
                  class="flex w-full items-center gap-2 px-2.5 py-1.5 text-left transition-colors hover:bg-accent/30"
                  onclick={() => toggleService(c, s)}
                >
                  <span class="text-muted-foreground/50">
                    {#if s.expanded}
                      <ChevronDown class="h-3 w-3" />
                    {:else}
                      <ChevronRight class="h-3 w-3" />
                    {/if}
                  </span>
                  <span class="flex-1 truncate font-mono text-[11px]">{s.name}</span>
                  <span
                    class={cn(
                      'shrink-0 rounded-full px-1.5 py-0.5 text-[10px] tabular-nums',
                      s.running === s.desired
                        ? 'bg-status-ok/15 text-status-ok'
                        : 'bg-status-warn/15 text-status-warn'
                    )}
                  >
                    {s.running}/{s.desired}
                  </span>
                </button>

                {#if s.expanded}
                  <div class="ml-5 border-l border-border/40">
                    {#if s.loading}
                      <div class="flex items-center gap-2 px-3 py-1.5 text-[11px] text-muted-foreground">
                        <Loader2 class="h-3 w-3 animate-spin" /> Loading…
                      </div>
                    {/if}
                    {#each s.tasks ?? [] as t (t.arn)}
                      <button
                        class={cn(
                          'flex w-full items-center gap-2 rounded-sm px-2.5 py-1.5 text-left transition-colors hover:bg-accent/30',
                          activeTask?.arn === t.arn && 'bg-primary/10 hover:bg-primary/[0.13]'
                        )}
                        onclick={() => selectTask(t)}
                      >
                        <CircleDot
                          class={cn(
                            'h-3 w-3 shrink-0',
                            t.lastStatus === 'RUNNING' ? 'text-status-ok' : 'text-muted-foreground/40'
                          )}
                        />
                        <span class="flex-1 truncate font-mono text-[10px] text-muted-foreground">
                          {taskShortId(t.arn)}
                        </span>
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

    <!-- Right panel: task detail + containers + terminal -->
    <div class="flex min-w-0 flex-1 flex-col">
      {#if activeTask}
        {@const task = activeTask}
        <!-- Task header -->
        <div class="flex shrink-0 items-center gap-3 border-b border-border bg-card/20 px-4 py-3">
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <CircleDot
                class={cn(
                  'h-3.5 w-3.5 shrink-0',
                  task.lastStatus === 'RUNNING' ? 'text-status-ok' : 'text-muted-foreground/40'
                )}
              />
              <p class="truncate font-mono text-sm font-semibold">{taskShortId(task.arn)}</p>
              <Badge
                variant={task.lastStatus === 'RUNNING' ? 'ok' : 'muted'}
                class="shrink-0 text-[10px]"
              >
                {task.lastStatus}
              </Badge>
            </div>
            <p class="mt-0.5 truncate pl-5 text-[11px] text-muted-foreground">
              {task.launchType} · {task.cluster}
            </p>
          </div>
        </div>

        <!-- Containers list -->
        <div class="min-h-0 flex-1 overflow-auto p-4">
          {#if loadingContainers}
            <div class="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 class="h-4 w-4 animate-spin" /> Loading containers…
            </div>
          {:else if containers.length === 0}
            <p class="text-sm text-muted-foreground/60">No containers found.</p>
          {:else}
            <div class="space-y-2">
              {#each containers as c (c.name)}
                {@const isActive = termContainer?.container.name === c.name}
                <div
                  class={cn(
                    'group rounded-lg border bg-card transition-colors',
                    isActive
                      ? 'border-primary/30 bg-primary/5'
                      : 'border-border hover:border-border/80 hover:bg-card/80'
                  )}
                >
                  <!-- Left accent bar for active -->
                  <div class="flex items-stretch overflow-hidden rounded-lg">
                    {#if isActive}
                      <div class="w-0.5 shrink-0 bg-primary/60"></div>
                    {/if}
                    <div class="flex min-w-0 flex-1 items-center gap-3 p-3">
                      <!-- Icon -->
                      <div class={cn(
                        'flex h-8 w-8 shrink-0 items-center justify-center rounded-md',
                        c.lastStatus === 'RUNNING' ? 'bg-status-ok/10' : 'bg-muted/50'
                      )}>
                        <Box class={cn(
                          'h-4 w-4',
                          c.lastStatus === 'RUNNING' ? 'text-status-ok' : 'text-muted-foreground/40'
                        )} />
                      </div>
                      <!-- Info -->
                      <div class="min-w-0 flex-1">
                        <div class="flex items-center gap-2">
                          <span class="font-mono text-sm font-semibold">{c.name}</span>
                          <Badge
                            variant={c.lastStatus === 'RUNNING' ? 'ok' : 'muted'}
                            class="text-[10px]"
                          >
                            {c.lastStatus}
                          </Badge>
                          {#if c.health}
                            <Badge
                              variant={c.health === 'HEALTHY' ? 'ok' : 'warn'}
                              class="text-[10px]"
                            >
                              {c.health}
                            </Badge>
                          {/if}
                        </div>
                        <p class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground">
                          {c.image || '—'}
                        </p>
                      </div>
                      <!-- Exec button -->
                      {#if c.lastStatus === 'RUNNING'}
                        <Button
                          size="sm"
                          variant={isActive ? 'secondary' : 'outline'}
                          onclick={() => execContainer(task, c)}
                          class="shrink-0"
                        >
                          <Terminal class="h-3.5 w-3.5" />
                          Exec
                        </Button>
                      {/if}
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>

        <!-- ECS exec terminal -->
        {#if termContainer}
          {@const { task: t, container } = termContainer}
          {@const ptyId = `ecs-${taskShortId(t.arn)}-${container.name}-${termKey}`}
          <div class="h-64 shrink-0 border-t border-border">
            <PtyTerminal
              {ptyId}
              title="exec · {container.name} · {taskShortId(t.arn)}"
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
        <div class="flex flex-1 flex-col items-center justify-center gap-3 text-center">
          <div class="rounded-full bg-muted/50 p-4">
            <Boxes class="h-8 w-8 text-muted-foreground/30" />
          </div>
          <div>
            <p class="text-sm font-medium text-muted-foreground">No task selected</p>
            <p class="mt-1 text-xs text-muted-foreground/50">
              Expand a cluster → service → task in the sidebar
            </p>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
