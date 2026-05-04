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
    Server,
    Copy,
    MapPin,
    Cpu,
    Network,
    Globe,
    Tag,
    ChevronDown,
    ChevronUp,
    Loader2,
    PlugZap
  } from 'lucide-svelte';
  import { cn } from '$lib/utils';

  let filter = $state('');
  let selected = $state<Instance | null>(null);
  let tagsOpen = $state(true);
  let termInstance = $state<Instance | null>(null);
  let termKey = $state(0);
  let copiedIp = $state<string | null>(null);

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

  function stateColor(s: string) {
    if (s === 'running') return 'text-status-ok';
    if (s === 'pending' || s === 'stopping') return 'text-status-warn';
    if (s === 'terminated' || s === 'shutting-down') return 'text-status-error';
    return 'text-muted-foreground/50';
  }

  function stateDot(s: string) {
    if (s === 'running') return 'bg-status-ok shadow-[0_0_8px_theme(colors.status.ok/60%)]';
    if (s === 'pending' || s === 'stopping') return 'bg-status-warn';
    if (s === 'terminated' || s === 'shutting-down') return 'bg-status-error';
    return 'bg-muted-foreground/30';
  }

  function statePill(s: string) {
    if (s === 'running') return 'bg-status-ok/12 text-status-ok border-status-ok/20';
    if (s === 'pending' || s === 'stopping') return 'bg-status-warn/12 text-status-warn border-status-warn/20';
    if (s === 'terminated' || s === 'shutting-down') return 'bg-status-error/12 text-status-error border-status-error/20';
    return 'bg-muted/50 text-muted-foreground border-border';
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

<div class="flex h-full flex-col">
  <!-- Toolbar -->
  <div class="flex h-11 shrink-0 items-center gap-3 border-b border-border bg-card/30 px-4">
    <Server class="h-4 w-4 text-muted-foreground/60" />
    <span class="text-sm font-semibold">EC2 Instances</span>
    {#if $instances.length > 0}
      <div class="flex items-center gap-1.5">
        <span class="rounded-full bg-muted px-2 py-0.5 text-[11px] tabular-nums text-muted-foreground">
          {$instances.length} total
        </span>
        {#if runningCount > 0}
          <span class="rounded-full bg-status-ok/15 px-2 py-0.5 text-[11px] tabular-nums text-status-ok">
            {runningCount} running
          </span>
        {/if}
      </div>
    {/if}
    <Button
      variant="ghost"
      size="sm"
      onclick={refresh}
      disabled={$loading.instances}
      class="ml-auto h-8 text-muted-foreground hover:text-foreground"
    >
      <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.instances ? 'animate-spin' : '')} />
      Refresh
    </Button>
  </div>

  <!-- Masterlist layout -->
  <div class="flex min-h-0 flex-1">
    <!-- LEFT: instance list -->
    <div class="flex w-80 shrink-0 flex-col border-r border-border bg-sidebar-background">
      <!-- Search -->
      <div class="border-b border-border/60 p-2.5">
        <div class="relative">
          <Search class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground/40" />
          <input
            class="h-8 w-full rounded-md border border-border/60 bg-background/60 pl-8 pr-3 text-xs placeholder:text-muted-foreground/40 focus:border-primary/50 focus:bg-background focus:outline-none focus:ring-2 focus:ring-primary/10"
            placeholder="Search instances…"
            bind:value={filter}
          />
        </div>
        {#if filter}
          <p class="mt-1.5 px-0.5 text-[10px] text-muted-foreground/60">
            {filtered.length} of {$instances.length} results
          </p>
        {/if}
      </div>

      <!-- Instance list -->
      <div class="min-h-0 flex-1 overflow-auto">
        {#if $loading.instances}
          <div class="flex flex-col items-center justify-center gap-2.5 py-12 text-center">
            <Loader2 class="h-5 w-5 animate-spin text-muted-foreground/40" />
            <p class="text-xs text-muted-foreground/60">Loading instances…</p>
          </div>
        {:else if filtered.length === 0}
          <div class="flex flex-col items-center justify-center gap-2 py-12 text-center">
            <Server class="h-8 w-8 text-muted-foreground/20" />
            <p class="text-xs text-muted-foreground/60">
              {filter ? 'No instances match' : 'No instances found'}
            </p>
          </div>
        {:else}
          {#each filtered as inst (inst.id)}
            {@const isSelected = selected?.id === inst.id}
            <button
              class={cn(
                'group relative w-full border-b border-border/30 px-3.5 py-3 text-left transition-colors',
                isSelected ? 'bg-accent/60' : 'hover:bg-accent/25'
              )}
              onclick={() => { selected = inst; tagsOpen = true; }}
            >
              <!-- Selection accent -->
              <div class={cn(
                'absolute inset-y-0 left-0 w-0.5 transition-all',
                isSelected ? stateDot(inst.state).includes('ok') ? 'bg-status-ok' : stateDot(inst.state).includes('warn') ? 'bg-status-warn' : stateDot(inst.state).includes('error') ? 'bg-status-error' : 'bg-primary' : 'bg-transparent'
              )}></div>

              <div class="flex items-start gap-2.5">
                <!-- Status dot -->
                <div class={cn('mt-[5px] h-2 w-2 shrink-0 rounded-full', stateDot(inst.state))}></div>

                <!-- Info -->
                <div class="min-w-0 flex-1">
                  <p class={cn(
                    'truncate text-[13px] font-medium leading-tight',
                    isSelected ? 'text-foreground' : 'text-foreground/90'
                  )}>
                    {inst.name ?? inst.id}
                  </p>
                  {#if inst.name}
                    <p class="mt-0.5 truncate font-mono text-[10px] text-muted-foreground/60">
                      {inst.id}
                    </p>
                  {/if}
                  <div class="mt-1.5 flex items-center gap-1.5">
                    <span class="rounded bg-muted/70 px-1.5 py-0.5 font-mono text-[10px] text-foreground/60">
                      {inst.instanceType}
                    </span>
                    {#if inst.az}
                      <span class="text-[10px] text-muted-foreground/50">{inst.az}</span>
                    {/if}
                  </div>
                </div>

                <!-- State text (right-aligned) -->
                <span class={cn('shrink-0 text-[10px] font-medium', stateColor(inst.state))}>
                  {inst.state}
                </span>
              </div>
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <!-- RIGHT: detail pane -->
    <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
      {#if selected}
        {@const inst = selected}
        <div class="min-h-0 flex-1 overflow-auto">
          <!-- Hero header -->
          <div class="border-b border-border/60 px-8 py-6">
            <div class="flex items-start justify-between gap-4">
              <div class="min-w-0 flex-1">
                <h1 class="truncate text-[22px] font-bold tracking-tight">
                  {inst.name ?? inst.id}
                </h1>
                {#if inst.name}
                  <p class="mt-1 font-mono text-xs text-muted-foreground/60">{inst.id}</p>
                {/if}
              </div>
              <!-- Status pill -->
              <span class={cn(
                'shrink-0 mt-0.5 inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-semibold',
                statePill(inst.state)
              )}>
                <span class={cn('h-1.5 w-1.5 rounded-full', stateDot(inst.state))}></span>
                {inst.state}
              </span>
            </div>

            <!-- Quick-info chips -->
            <div class="mt-4 flex flex-wrap items-center gap-2">
              <span class="inline-flex items-center gap-1.5 rounded-md border border-border/60 bg-muted/40 px-2.5 py-1 text-xs">
                <Cpu class="h-3 w-3 text-muted-foreground/60" />
                <span class="font-mono font-medium">{inst.instanceType}</span>
              </span>
              {#if inst.az}
                <span class="inline-flex items-center gap-1.5 rounded-md border border-border/60 bg-muted/40 px-2.5 py-1 text-xs text-muted-foreground">
                  <MapPin class="h-3 w-3" />
                  {inst.az}
                </span>
              {/if}
              {#if inst.vpcId}
                <span class="inline-flex items-center gap-1 rounded-md border border-border/60 bg-muted/40 px-2.5 py-1 font-mono text-[11px] text-muted-foreground">
                  {inst.vpcId}
                </span>
              {/if}
            </div>
          </div>

          <!-- Network section -->
          {#if inst.privateIp || inst.publicIp}
            <div class="border-b border-border/40 px-8 py-5">
              <h3 class="mb-3 text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/50">
                Network
              </h3>
              <div class="grid grid-cols-2 gap-3">
                {#if inst.privateIp}
                  <button
                    onclick={() => copyIp(inst.privateIp!)}
                    class="group/ip relative rounded-xl border border-border/60 bg-card px-4 py-3.5 text-left transition-all hover:border-primary/30 hover:bg-primary/5 hover:shadow-sm"
                  >
                    <div class="flex items-center gap-1.5 text-[10px] text-muted-foreground/60">
                      <Network class="h-3 w-3" />
                      <span class="uppercase tracking-wider">Private IP</span>
                    </div>
                    <p class="mt-1.5 font-mono text-sm font-semibold">
                      {copiedIp === inst.privateIp ? '✓ Copied!' : inst.privateIp}
                    </p>
                    <Copy class="absolute right-3 top-3 h-3.5 w-3.5 text-transparent transition-colors group-hover/ip:text-muted-foreground/40" />
                  </button>
                {/if}
                {#if inst.publicIp}
                  <button
                    onclick={() => copyIp(inst.publicIp!)}
                    class="group/ip relative rounded-xl border border-border/60 bg-card px-4 py-3.5 text-left transition-all hover:border-primary/30 hover:bg-primary/5 hover:shadow-sm"
                  >
                    <div class="flex items-center gap-1.5 text-[10px] text-muted-foreground/60">
                      <Globe class="h-3 w-3" />
                      <span class="uppercase tracking-wider">Public IP</span>
                    </div>
                    <p class="mt-1.5 font-mono text-sm font-semibold">
                      {copiedIp === inst.publicIp ? '✓ Copied!' : inst.publicIp}
                    </p>
                    <Copy class="absolute right-3 top-3 h-3.5 w-3.5 text-transparent transition-colors group-hover/ip:text-muted-foreground/40" />
                  </button>
                {/if}
              </div>
            </div>
          {/if}

          <!-- Tags section -->
          {#if Object.keys(inst.tags).length > 0}
            <div class="border-b border-border/40 px-8 py-5">
              <button
                onclick={() => (tagsOpen = !tagsOpen)}
                class="flex w-full items-center justify-between text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/50 transition-colors hover:text-muted-foreground/80"
              >
                <span class="flex items-center gap-1.5">
                  <Tag class="h-3 w-3" />
                  Tags ({Object.keys(inst.tags).length})
                </span>
                {#if tagsOpen}
                  <ChevronUp class="h-3 w-3" />
                {:else}
                  <ChevronDown class="h-3 w-3" />
                {/if}
              </button>
              {#if tagsOpen}
                <div class="mt-3 flex flex-wrap gap-1.5">
                  {#each Object.entries(inst.tags) as [k, v] (k)}
                    <span class="inline-flex items-center gap-1 rounded-md border border-border/50 bg-muted/30 px-2 py-1 font-mono text-[11px]">
                      <span class="text-muted-foreground/70">{k}:</span>
                      <span class="font-medium">{v}</span>
                    </span>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}

          <!-- SSM Connect CTA -->
          {#if inst.state === 'running'}
            <div class="px-8 py-5">
              <h3 class="mb-3 text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/50">
                Actions
              </h3>
              <Button
                class="gap-2"
                onclick={() => connectSsm(inst)}
              >
                <PlugZap class="h-4 w-4" />
                Open SSM Shell
              </Button>
            </div>
          {/if}
        </div>

        <!-- SSM Terminal -->
        {#if termInstance}
          {@const tinst = termInstance}
          {@const ptyId = `ssm-${tinst.id}-${termKey}`}
          <div class="h-72 shrink-0 border-t border-border">
            <PtyTerminal
              {ptyId}
              title="SSM · {tinst.name ?? tinst.id} · {tinst.id}"
              onReady={async (rows, cols) => {
                await ipc.ptyOpenSsm(ptyId, tinst.id, get(profile), get(region), rows, cols);
              }}
              onClose={() => (termInstance = null)}
            />
          </div>
        {/if}
      {:else}
        <!-- Empty state -->
        <div class="flex flex-1 flex-col items-center justify-center gap-4 text-center">
          <div class="flex h-16 w-16 items-center justify-center rounded-2xl border border-border/60 bg-card">
            <Server class="h-7 w-7 text-muted-foreground/30" />
          </div>
          <div>
            <p class="text-sm font-semibold text-foreground/70">Select an instance</p>
            <p class="mt-1 text-xs text-muted-foreground/50">
              Choose from the list on the left to view details and connect
            </p>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
