<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { ipc } from '$lib/ipc';
  import { instances, loading, profile, region } from '$lib/stores/aws';
  import type { Instance } from '$lib/types';
  import PtyTerminal from '$lib/components/pty-terminal.svelte';
  import { Button } from '$lib/components/ui';
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
    ChevronUp,
    ArrowUp,
    ArrowDown,
    ArrowUpDown,
    MapPin
  } from 'lucide-svelte';
  import { cn } from '$lib/utils';

  let filter = $state('');
  let selected = $state<Instance | null>(null);
  let tagsOpen = $state(true);
  let termInstance = $state<Instance | null>(null);
  let termKey = $state(0);

  // Sorting
  let sortKey = $state<keyof Instance | null>(null);
  let sortDir = $state<'asc' | 'desc'>('asc');

  function toggleSort(key: keyof Instance) {
    if (sortKey === key) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortKey = key;
      sortDir = 'asc';
    }
  }

  async function refresh() {
    loading.update((l) => ({ ...l, instances: true }));
    selected = null;
    termInstance = null;
    try {
      instances.set(await ipc.listInstances(get(profile), get(region)));
    } finally {
      loading.update((l) => ({ ...l, instances: false }));
    }
  }

  onMount(refresh);

  function connectSsm(inst: Instance) {
    termInstance = inst;
    termKey += 1;
  }

  function stateDotClass(s: string): string {
    if (s === 'running') return 'bg-status-ok shadow-[0_0_6px_theme(colors.status.ok/0.5)]';
    if (s === 'pending' || s === 'stopping') return 'bg-status-warn';
    if (s === 'terminated' || s === 'shutting-down') return 'bg-status-error';
    return 'bg-muted-foreground/40';
  }

  function stateTextClass(s: string): string {
    if (s === 'running') return 'text-status-ok';
    if (s === 'pending' || s === 'stopping') return 'text-status-warn';
    if (s === 'terminated' || s === 'shutting-down') return 'text-status-error';
    return 'text-muted-foreground';
  }

  let filtered = $derived.by(() => {
    const f = filter.trim().toLowerCase();
    let rows = f
      ? $instances.filter((i) =>
          [i.id, i.name, i.state, i.instanceType, i.privateIp, i.az]
            .filter(Boolean)
            .some((v) => v!.toLowerCase().includes(f))
        )
      : $instances;

    if (sortKey) {
      const k = sortKey;
      rows = [...rows].sort((a, b) => {
        const av = String(a[k] ?? '');
        const bv = String(b[k] ?? '');
        const cmp = av.localeCompare(bv);
        return sortDir === 'asc' ? cmp : -cmp;
      });
    }
    return rows;
  });

  let copiedIp = $state<string | null>(null);
  async function copyIp(ip: string) {
    try {
      const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
      await writeText(ip);
    } catch {
      try { await navigator.clipboard.writeText(ip); } catch { return; }
    }
    copiedIp = ip;
    setTimeout(() => { if (copiedIp === ip) copiedIp = null; }, 1200);
  }
</script>

<div class="flex h-full flex-col">
  <!-- Toolbar -->
  <div class="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card/40 px-4">
    <div class="flex items-center gap-2">
      <Server class="h-4 w-4 text-muted-foreground" />
      <h1 class="text-sm font-semibold">EC2 Instances</h1>
      {#if $instances.length > 0}
        <span class="rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium tabular-nums text-muted-foreground">
          {filter ? `${filtered.length}/` : ''}{$instances.length}
        </span>
      {/if}
    </div>

    <!-- Search -->
    <div class="relative ml-2">
      <Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground/60" />
      <input
        class="h-8 w-52 rounded-md border border-border bg-background pl-8 pr-3 text-xs placeholder:text-muted-foreground/50 focus:border-primary/40 focus:outline-none focus:ring-2 focus:ring-primary/10"
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

  <!-- Main split: table + detail panel (always rendered) -->
  <div class="flex min-h-0 flex-1">
    <!-- Table area -->
    <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <div class="min-h-0 flex-1 overflow-auto">
        <table class="w-full caption-bottom">
          <thead class="sticky top-0 z-10 border-b border-border bg-card/90 backdrop-blur-sm">
            <tr>
              {#each [
                { key: 'name', label: 'Instance', sortable: true },
                { key: 'state', label: 'State', sortable: true },
                { key: 'instanceType', label: 'Type', sortable: true },
                { key: 'privateIp', label: 'Private IP', sortable: false },
                { key: 'az', label: 'AZ', sortable: true }
              ] as col (col.key)}
                <th
                  class={cn(
                    'h-10 px-3 text-left text-[11px] font-medium uppercase tracking-wider text-muted-foreground/70 select-none',
                    col.sortable && 'cursor-pointer hover:text-foreground'
                  )}
                  onclick={() => col.sortable && toggleSort(col.key as keyof Instance)}
                >
                  <div class="flex items-center gap-1">
                    {col.label}
                    {#if col.sortable}
                      {#if sortKey === col.key}
                        {#if sortDir === 'asc'}
                          <ArrowUp class="h-3 w-3 text-primary" />
                        {:else}
                          <ArrowDown class="h-3 w-3 text-primary" />
                        {/if}
                      {:else}
                        <ArrowUpDown class="h-3 w-3 opacity-30" />
                      {/if}
                    {/if}
                  </div>
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each filtered as inst (inst.id)}
              {@const isSelected = selected?.id === inst.id}
              <tr
                class={cn(
                  'group border-b border-border/50 transition-colors cursor-pointer',
                  isSelected
                    ? 'bg-primary/10 hover:bg-primary/[0.13]'
                    : 'hover:bg-muted/40'
                )}
                onclick={() => { selected = inst; tagsOpen = true; }}
              >
                <!-- Name + ID stacked -->
                <td class="px-3 py-2.5">
                  <div class="flex items-center gap-2">
                    {#if isSelected}
                      <div class="h-full w-0.5 self-stretch rounded-full bg-primary/60 -ml-3 mr-1"></div>
                    {/if}
                    <div class="min-w-0">
                      <p class="truncate text-sm font-medium leading-tight">
                        {inst.name ?? inst.id}
                      </p>
                      {#if inst.name}
                        <p class="truncate font-mono text-[10px] text-muted-foreground leading-tight mt-0.5">
                          {inst.id}
                        </p>
                      {/if}
                    </div>
                  </div>
                </td>
                <!-- State dot + text -->
                <td class="px-3 py-2.5">
                  <div class="flex items-center gap-1.5">
                    <span class={cn('h-1.5 w-1.5 rounded-full shrink-0', stateDotClass(inst.state))}></span>
                    <span class={cn('text-xs font-medium', stateTextClass(inst.state))}>
                      {inst.state}
                    </span>
                  </div>
                </td>
                <!-- Type badge -->
                <td class="px-3 py-2.5">
                  <span class="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px] text-foreground/80">
                    {inst.instanceType}
                  </span>
                </td>
                <!-- IP copyable -->
                <td class="px-3 py-2.5">
                  {#if inst.privateIp}
                    <button
                      onclick={(e) => { e.stopPropagation(); copyIp(inst.privateIp!); }}
                      class="font-mono text-xs text-muted-foreground transition-colors hover:text-foreground"
                      title="Click to copy"
                    >
                      {copiedIp === inst.privateIp ? '✓ copied' : inst.privateIp}
                    </button>
                  {:else}
                    <span class="text-xs text-muted-foreground/40">—</span>
                  {/if}
                </td>
                <!-- AZ -->
                <td class="px-3 py-2.5">
                  <span class="font-mono text-xs text-muted-foreground">{inst.az ?? '—'}</span>
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="5" class="py-16 text-center">
                  {#if $loading.instances}
                    <div class="flex items-center justify-center gap-2 text-sm text-muted-foreground">
                      <RefreshCw class="h-4 w-4 animate-spin" />
                      Loading instances…
                    </div>
                  {:else}
                    <p class="text-sm text-muted-foreground">No instances found</p>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>

    <!-- Detail panel — always visible -->
    <div class="flex w-72 shrink-0 flex-col border-l border-border bg-card/20">
      {#if selected}
        {@const inst = selected}
        <!-- Panel header -->
        <div class="flex items-start justify-between gap-2 border-b border-border px-4 py-3">
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <span class={cn('h-2 w-2 shrink-0 rounded-full', stateDotClass(inst.state))}></span>
              <p class="truncate font-mono text-sm font-semibold leading-snug">
                {inst.name ?? inst.id}
              </p>
            </div>
            {#if inst.name}
              <p class="mt-0.5 truncate pl-4 font-mono text-[10px] text-muted-foreground">{inst.id}</p>
            {/if}
          </div>
          <button
            onclick={() => { selected = null; }}
            class="mt-0.5 shrink-0 rounded p-0.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <X class="h-3.5 w-3.5" />
          </button>
        </div>

        <!-- Connect button (if running) -->
        {#if inst.state === 'running'}
          <div class="border-b border-border px-4 py-3">
            <Button
              size="sm"
              class="w-full"
              onclick={() => connectSsm(inst)}
            >
              <Terminal class="h-3.5 w-3.5" />
              Open SSM Shell
            </Button>
          </div>
        {/if}

        <!-- Detail rows -->
        <div class="min-h-0 flex-1 overflow-auto px-4 py-3">
          <div class="space-y-3">

            <!-- State + type -->
            <div class="rounded-lg border border-border/60 bg-muted/20 px-3 py-2.5 text-xs">
              <div class="flex items-center justify-between">
                <span class="text-muted-foreground">State</span>
                <span class={cn('font-medium', stateTextClass(inst.state))}>{inst.state}</span>
              </div>
              <div class="mt-1.5 flex items-center justify-between">
                <span class="text-muted-foreground">Type</span>
                <span class="rounded bg-muted/80 px-1.5 py-0.5 font-mono text-[11px]">{inst.instanceType}</span>
              </div>
            </div>

            <!-- Network -->
            <div>
              <p class="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/60">Network</p>
              <div class="space-y-1.5">
                {#if inst.privateIp}
                  <div class="flex items-center gap-2 rounded-md px-2 py-1.5 transition-colors hover:bg-muted/40">
                    <Network class="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" />
                    <div class="min-w-0 flex-1">
                      <p class="text-[10px] text-muted-foreground">Private IP</p>
                      <button
                        onclick={() => copyIp(inst.privateIp!)}
                        class="font-mono text-xs hover:text-primary transition-colors"
                        title="Click to copy"
                      >
                        {copiedIp === inst.privateIp ? '✓ copied' : inst.privateIp}
                      </button>
                    </div>
                  </div>
                {/if}
                {#if inst.publicIp}
                  <div class="flex items-center gap-2 rounded-md px-2 py-1.5 transition-colors hover:bg-muted/40">
                    <Globe class="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" />
                    <div class="min-w-0 flex-1">
                      <p class="text-[10px] text-muted-foreground">Public IP</p>
                      <button
                        onclick={() => copyIp(inst.publicIp!)}
                        class="font-mono text-xs hover:text-primary transition-colors"
                        title="Click to copy"
                      >
                        {copiedIp === inst.publicIp ? '✓ copied' : inst.publicIp}
                      </button>
                    </div>
                  </div>
                {/if}
                {#if inst.az}
                  <div class="flex items-center gap-2 rounded-md px-2 py-1.5">
                    <MapPin class="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" />
                    <div class="min-w-0 flex-1">
                      <p class="text-[10px] text-muted-foreground">Availability Zone</p>
                      <p class="font-mono text-xs">{inst.az}</p>
                    </div>
                  </div>
                {/if}
                {#if inst.vpcId}
                  <div class="flex items-center gap-2 rounded-md px-2 py-1.5">
                    <Server class="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" />
                    <div class="min-w-0 flex-1">
                      <p class="text-[10px] text-muted-foreground">VPC</p>
                      <p class="truncate font-mono text-[11px]">{inst.vpcId}</p>
                    </div>
                  </div>
                {/if}
              </div>
            </div>

            <!-- Tags -->
            {#if Object.keys(inst.tags).length > 0}
              <div>
                <button
                  onclick={() => (tagsOpen = !tagsOpen)}
                  class="mb-1.5 flex w-full items-center gap-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/60 transition-colors hover:text-muted-foreground"
                >
                  <Tag class="h-3 w-3" />
                  Tags ({Object.keys(inst.tags).length})
                  <span class="ml-auto">
                    {#if tagsOpen}<ChevronUp class="h-3 w-3" />{:else}<ChevronDown class="h-3 w-3" />{/if}
                  </span>
                </button>
                {#if tagsOpen}
                  <div class="space-y-1">
                    {#each Object.entries(inst.tags) as [k, v] (k)}
                      <div class="flex items-baseline gap-2 rounded-md bg-muted/40 px-2 py-1">
                        <span class="shrink-0 font-mono text-[10px] text-muted-foreground">{k}</span>
                        <span class="min-w-0 truncate font-mono text-[11px]">{v}</span>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}

          </div>
        </div>
      {:else}
        <!-- Empty state -->
        <div class="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
          <div class="rounded-full bg-muted/50 p-3">
            <Server class="h-6 w-6 text-muted-foreground/40" />
          </div>
          <div>
            <p class="text-sm font-medium text-muted-foreground">No instance selected</p>
            <p class="mt-1 text-xs text-muted-foreground/50">Click a row to view details</p>
          </div>
        </div>
      {/if}
    </div>
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
