<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { ipc } from '$lib/ipc';
  import { instances, loading, profile, region } from '$lib/stores/aws';
  import type { Instance } from '$lib/types';
  import DataTable, { type Column } from '$lib/components/data-table.svelte';
  import PtyTerminal from '$lib/components/pty-terminal.svelte';
  import { Badge, Button, Input } from '$lib/components/ui';
  import {
    RefreshCw,
    Search,
    Terminal,
    X,
    Globe,
    Server,
    Network,
    Tag,
    ChevronDown,
    ChevronUp
  } from 'lucide-svelte';

  let filter = $state('');
  let selected = $state<Instance | null>(null);
  let detail = $state<unknown>(null);
  let tagsOpen = $state(false);

  // SSM terminal
  let termInstance = $state<Instance | null>(null);
  let termKey = $state(0);

  async function refresh() {
    loading.update((l) => ({ ...l, instances: true }));
    try {
      instances.set(await ipc.listInstances(get(profile), get(region)));
    } finally {
      loading.update((l) => ({ ...l, instances: false }));
    }
  }

  onMount(refresh);

  async function select(row: Instance) {
    selected = row;
    tagsOpen = false;
    detail = await ipc.describeInstance(row.id, get(profile), get(region));
  }

  function connectSsm(inst: Instance) {
    termInstance = inst;
    termKey += 1;
  }

  function stateBadge(s: string): 'ok' | 'warn' | 'error' | 'muted' {
    if (s === 'running') return 'ok';
    if (s === 'pending' || s === 'stopping') return 'warn';
    if (s === 'terminated' || s === 'shutting-down') return 'error';
    return 'muted';
  }

  const columns: Column<Instance>[] = [
    {
      key: 'name',
      header: 'Name',
      sortable: true,
      accessor: (r) => r.name ?? '',
      cell: undefined
    },
    {
      key: 'id',
      header: 'Instance ID',
      sortable: true,
      accessor: (r) => r.id
    },
    { key: 'state', header: 'State', sortable: true, accessor: (r) => r.state },
    { key: 'instanceType', header: 'Type', sortable: true, accessor: (r) => r.instanceType },
    { key: 'privateIp', header: 'Private IP', accessor: (r) => r.privateIp ?? '—' },
    { key: 'az', header: 'AZ', accessor: (r) => r.az ?? '—' }
  ];

  let selectedKey = $derived(selected?.id ?? null);
  let instanceCount = $derived($instances.length);
  let filteredCount = $derived(
    filter
      ? $instances.filter((i) =>
          [i.id, i.name, i.state, i.instanceType, i.privateIp, i.az]
            .filter(Boolean)
            .some((v) => v!.toLowerCase().includes(filter.toLowerCase()))
        ).length
      : instanceCount
  );
</script>

<div class="flex h-full flex-col">
  <!-- Toolbar -->
  <div class="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card/40 px-4">
    <div class="flex items-center gap-2">
      <Server class="h-4 w-4 text-muted-foreground" />
      <h1 class="text-sm font-semibold">EC2 Instances</h1>
      <span class="rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium tabular-nums text-muted-foreground">
        {filter ? `${filteredCount}/` : ''}{instanceCount}
      </span>
    </div>

    <div class="relative ml-2 w-56">
      <Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input
        class="h-8 pl-8 text-xs"
        placeholder="Filter instances…"
        bind:value={filter}
      />
    </div>

    <Button
      variant="outline"
      size="sm"
      onclick={refresh}
      disabled={$loading.instances}
      class="ml-auto h-8"
    >
      <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.instances ? 'animate-spin' : '')} />
      Refresh
    </Button>
  </div>

  <!-- Main content: table + detail panel -->
  <div class="flex min-h-0 flex-1">
    <!-- Instance table -->
    <div class="flex min-w-0 flex-1 flex-col p-3">
      <DataTable
        data={$instances}
        {columns}
        {filter}
        {selectedKey}
        rowKey={(r) => r.id}
        onRowClick={select}
        emptyLabel={$loading.instances ? 'Loading instances…' : 'No instances found'}
      />
    </div>

    <!-- Detail panel -->
    {#if selected}
      {@const inst = selected}
      <div class="flex w-72 shrink-0 flex-col border-l border-border bg-card/30">
        <!-- Header -->
        <div class="flex items-start justify-between gap-2 border-b border-border p-4">
          <div class="min-w-0">
            <p class="truncate font-mono text-sm font-semibold">
              {inst.name ?? inst.id}
            </p>
            <p class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground">{inst.id}</p>
          </div>
          <button
            onclick={() => { selected = null; detail = null; }}
            class="mt-0.5 shrink-0 rounded-md p-0.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <!-- Metadata -->
        <div class="overflow-auto flex-1 p-4">
          <div class="space-y-4">
            <!-- Status row -->
            <div class="flex items-center gap-2">
              <Badge variant={stateBadge(inst.state)} class="font-mono text-[11px]">
                {inst.state}
              </Badge>
              <span class="font-mono text-xs text-muted-foreground">{inst.instanceType}</span>
            </div>

            <!-- Key-value grid -->
            <div class="space-y-2.5">
              {#if inst.privateIp}
                <div class="flex items-start gap-2">
                  <Network class="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <div class="min-w-0">
                    <p class="text-[10px] uppercase tracking-wider text-muted-foreground">Private IP</p>
                    <p class="font-mono text-xs">{inst.privateIp}</p>
                  </div>
                </div>
              {/if}
              {#if inst.publicIp}
                <div class="flex items-start gap-2">
                  <Globe class="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <div class="min-w-0">
                    <p class="text-[10px] uppercase tracking-wider text-muted-foreground">Public IP</p>
                    <p class="font-mono text-xs">{inst.publicIp}</p>
                  </div>
                </div>
              {/if}
              {#if inst.az}
                <div class="flex items-start gap-2">
                  <Server class="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <div class="min-w-0">
                    <p class="text-[10px] uppercase tracking-wider text-muted-foreground">AZ / VPC</p>
                    <p class="font-mono text-xs">{inst.az}</p>
                    {#if inst.vpcId}
                      <p class="font-mono text-[11px] text-muted-foreground">{inst.vpcId}</p>
                    {/if}
                  </div>
                </div>
              {/if}
            </div>

            <!-- Tags -->
            {#if Object.keys(inst.tags).length > 0}
              <div>
                <button
                  onclick={() => (tagsOpen = !tagsOpen)}
                  class="flex w-full items-center gap-2 text-[10px] uppercase tracking-wider text-muted-foreground hover:text-foreground transition-colors"
                >
                  <Tag class="h-3 w-3" />
                  <span>Tags ({Object.keys(inst.tags).length})</span>
                  <span class="ml-auto">
                    {#if tagsOpen}<ChevronUp class="h-3 w-3" />{:else}<ChevronDown class="h-3 w-3" />{/if}
                  </span>
                </button>
                {#if tagsOpen}
                  <div class="mt-2 space-y-1">
                    {#each Object.entries(inst.tags) as [k, v] (k)}
                      <div class="flex items-baseline gap-2 rounded-md bg-muted/50 px-2 py-1">
                        <span class="shrink-0 font-mono text-[10px] text-muted-foreground">{k}</span>
                        <span class="min-w-0 truncate font-mono text-[11px]">{v}</span>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}

            <!-- Connect button -->
            {#if inst.state === 'running'}
              <Button
                size="sm"
                class="w-full"
                onclick={() => connectSsm(inst)}
              >
                <Terminal class="h-3.5 w-3.5" />
                Connect via SSM
              </Button>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </div>

  <!-- SSM Terminal panel -->
  {#if termInstance}
    {@const inst = termInstance}
    {@const ptyId = `ssm-${inst.id}-${termKey}`}
    <div class="h-64 shrink-0 border-t border-border">
      <PtyTerminal
        {ptyId}
        title="SSM · {inst.name ?? inst.id} · {inst.id}"
        onReady={async (rows, cols) => {
          await ipc.ptyOpenSsm(ptyId, inst.id, get(profile), get(region), rows, cols);
        }}
        onClose={() => (termInstance = null)}
      />
    </div>
  {/if}
</div>
