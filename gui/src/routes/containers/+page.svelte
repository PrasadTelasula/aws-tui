<script lang="ts">
  import { onMount } from 'svelte';
  import { ipc } from '$lib/ipc';
  import { clusters, loading, profile, region } from '$lib/stores/aws';
  import type { Cluster, Container, Service, Task } from '$lib/types';
  import PageHeader from '$lib/components/app-shell/page-header.svelte';
  import { Badge, Button } from '$lib/components/ui';
  import { ChevronRight, ChevronDown, RefreshCw, Terminal } from 'lucide-svelte';

  type ServiceNode = Service & { tasks?: Task[]; expanded?: boolean; loading?: boolean };
  type ClusterNode = Cluster & { services?: ServiceNode[]; expanded?: boolean; loading?: boolean };

  let tree = $state<ClusterNode[]>([]);
  let activeTask = $state<Task | null>(null);
  let containers = $state<Container[]>([]);

  async function refresh() {
    loading.update((l) => ({ ...l, clusters: true }));
    try {
      const list = await ipc.listClusters($profile, $region);
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
      const services = await ipc.listServices(c.name);
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
      s.tasks = await ipc.listTasks(c.name, s.name);
      s.loading = false;
    }
    tree = [...tree];
  }

  async function selectTask(t: Task) {
    activeTask = t;
    containers = await ipc.listContainers(t.arn);
  }
</script>

<div class="h-full space-y-4 overflow-auto px-6 py-5">
  <PageHeader title="Containers" subtitle="ECS clusters, services, tasks, and containers.">
    {#snippet actions()}
      <Button variant="outline" size="sm" onclick={refresh} disabled={$loading.clusters}>
        <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.clusters ? 'animate-spin' : '')} />
        Refresh
      </Button>
    {/snippet}
  </PageHeader>

  <div class="grid gap-4 lg:grid-cols-[1fr_1fr]">
    <div class="rounded-lg border border-border bg-card">
      <div class="border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
        Tree
      </div>
      <div class="p-2 text-sm">
        {#if tree.length === 0}
          <p class="p-4 text-center text-sm text-muted-foreground">
            {$loading.clusters ? 'Loading clusters…' : 'No clusters'}
          </p>
        {/if}
        {#each tree as c (c.arn)}
          <div>
            <button
              class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-muted"
              onclick={() => toggleCluster(c)}
            >
              {#if c.expanded}
                <ChevronDown class="h-3.5 w-3.5 text-muted-foreground" />
              {:else}
                <ChevronRight class="h-3.5 w-3.5 text-muted-foreground" />
              {/if}
              <span class="font-mono text-sm font-medium">{c.name}</span>
              <Badge variant="muted" class="ml-auto">{c.runningTasks} tasks</Badge>
            </button>
            {#if c.expanded}
              <div class="ml-4 border-l border-border pl-2">
                {#if c.loading}
                  <p class="p-2 text-xs text-muted-foreground">Loading services…</p>
                {/if}
                {#each c.services ?? [] as s (s.arn)}
                  <button
                    class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-muted"
                    onclick={() => toggleService(c, s)}
                  >
                    {#if s.expanded}
                      <ChevronDown class="h-3.5 w-3.5 text-muted-foreground" />
                    {:else}
                      <ChevronRight class="h-3.5 w-3.5 text-muted-foreground" />
                    {/if}
                    <span class="font-mono text-xs">{s.name}</span>
                    <Badge variant={s.running === s.desired ? 'ok' : 'warn'} class="ml-auto">
                      {s.running}/{s.desired}
                    </Badge>
                  </button>
                  {#if s.expanded}
                    <div class="ml-4 border-l border-border pl-2">
                      {#each s.tasks ?? [] as t (t.arn)}
                        <button
                          class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-muted"
                          onclick={() => selectTask(t)}
                        >
                          <Terminal class="h-3.5 w-3.5 text-muted-foreground" />
                          <span class="font-mono text-[11px]">{t.arn.split('/').pop()}</span>
                          <Badge variant={t.lastStatus === 'RUNNING' ? 'ok' : 'muted'} class="ml-auto">
                            {t.lastStatus}
                          </Badge>
                        </button>
                      {/each}
                    </div>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>

    <aside class="rounded-lg border border-border bg-card p-4">
      {#if activeTask}
        <div class="space-y-3">
          <div>
            <p class="font-mono text-sm font-semibold">{activeTask.arn.split('/').pop()}</p>
            <p class="text-xs text-muted-foreground">{activeTask.launchType} · {activeTask.lastStatus}</p>
          </div>
          <div>
            <p class="mb-2 text-xs font-medium text-muted-foreground">Containers</p>
            <div class="space-y-2">
              {#each containers as c (c.name)}
                <div class="rounded-md border border-border p-3">
                  <div class="flex items-center justify-between">
                    <span class="font-mono text-sm font-medium">{c.name}</span>
                    <Badge variant={c.lastStatus === 'RUNNING' ? 'ok' : 'muted'}>{c.lastStatus}</Badge>
                  </div>
                  <p class="mt-1 truncate font-mono text-[11px] text-muted-foreground">{c.image}</p>
                  <Button size="sm" class="mt-2">
                    <Terminal class="h-3.5 w-3.5" /> Exec
                  </Button>
                </div>
              {/each}
            </div>
          </div>
        </div>
      {:else}
        <p class="text-sm text-muted-foreground">Select a task to view its containers.</p>
      {/if}
    </aside>
  </div>
</div>
