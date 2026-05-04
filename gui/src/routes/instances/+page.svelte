<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { ipc } from '$lib/ipc';
  import { instances, loading, profile, region } from '$lib/stores/aws';
  import type { Instance } from '$lib/types';
  import PageHeader from '$lib/components/app-shell/page-header.svelte';
  import DataTable, { type Column } from '$lib/components/data-table.svelte';
  import PtyTerminal from '$lib/components/pty-terminal.svelte';
  import { Badge, Button, Input } from '$lib/components/ui';
  import { RefreshCw, Search, Terminal } from 'lucide-svelte';

  let filter = $state('');
  let selected = $state<Instance | null>(null);
  let detail = $state<unknown>(null);

  // SSM terminal state
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
    { key: 'id', header: 'Instance ID', sortable: true, accessor: (r) => r.id },
    { key: 'name', header: 'Name', sortable: true, accessor: (r) => r.name ?? '' },
    { key: 'state', header: 'State', sortable: true, accessor: (r) => r.state },
    { key: 'instanceType', header: 'Type', sortable: true, accessor: (r) => r.instanceType },
    { key: 'privateIp', header: 'Private IP', accessor: (r) => r.privateIp ?? '' },
    { key: 'az', header: 'AZ', accessor: (r) => r.az ?? '' }
  ];
</script>

<div class="flex h-full flex-col gap-4 px-6 py-5">
  <PageHeader
    title="EC2 Instances"
    subtitle="Browse instances in the selected profile and region."
  >
    {#snippet actions()}
      <Button variant="outline" size="sm" onclick={refresh} disabled={$loading.instances}>
        <RefreshCw class={'h-3.5 w-3.5 ' + ($loading.instances ? 'animate-spin' : '')} />
        Refresh
      </Button>
    {/snippet}
  </PageHeader>

  <div class="flex items-center gap-2">
    <div class="relative max-w-sm flex-1">
      <Search
        class="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
      />
      <Input class="pl-8" placeholder="Filter instances…" bind:value={filter} />
    </div>
  </div>

  <div class="grid min-h-0 flex-1 gap-4 lg:grid-cols-[2fr_1fr]">
    <DataTable
      data={$instances}
      {columns}
      {filter}
      rowKey={(r) => r.id}
      onRowClick={select}
      emptyLabel={$loading.instances ? 'Loading…' : 'No instances'}
    />

    <aside class="overflow-auto rounded-lg border border-border bg-card p-4">
      {#if selected}
        {@const inst = selected}
        <div class="space-y-3">
          <div>
            <p class="font-mono text-sm font-semibold">{inst.id}</p>
            <p class="text-xs text-muted-foreground">{inst.name ?? 'no name tag'}</p>
          </div>
          <div class="grid grid-cols-2 gap-y-2 text-xs">
            <span class="text-muted-foreground">State</span>
            <Badge variant={stateBadge(inst.state)}>{inst.state}</Badge>
            <span class="text-muted-foreground">Type</span>
            <span class="font-mono">{inst.instanceType}</span>
            <span class="text-muted-foreground">Private IP</span>
            <span class="font-mono">{inst.privateIp ?? '—'}</span>
            <span class="text-muted-foreground">Public IP</span>
            <span class="font-mono">{inst.publicIp ?? '—'}</span>
            <span class="text-muted-foreground">AZ</span>
            <span class="font-mono">{inst.az ?? '—'}</span>
            <span class="text-muted-foreground">VPC</span>
            <span class="font-mono">{inst.vpcId ?? '—'}</span>
          </div>
          {#if inst.state === 'running'}
            <Button size="sm" onclick={() => connectSsm(inst)}>
              <Terminal class="h-3.5 w-3.5" />
              Connect (SSM)
            </Button>
          {/if}
          {#if detail}
            <details class="mt-3">
              <summary class="cursor-pointer text-xs font-medium text-muted-foreground"
                >Raw JSON</summary
              >
              <pre
                class="mt-2 max-h-64 overflow-auto rounded-md bg-muted p-2 font-mono text-[11px] leading-relaxed">{JSON.stringify(
                  detail,
                  null,
                  2
                )}</pre>
            </details>
          {/if}
        </div>
      {:else}
        <p class="text-sm text-muted-foreground">Select an instance to view details.</p>
      {/if}
    </aside>
  </div>

  {#if termInstance}
    {@const inst = termInstance}
    {@const ptyId = `ssm-${inst.id}-${termKey}`}
    <div class="h-72 shrink-0">
      <PtyTerminal
        {ptyId}
        title="SSM · {inst.name ?? inst.id}"
        onReady={async (rows, cols) => {
          await ipc.ptyOpenSsm(ptyId, inst.id, get(profile), get(region), rows, cols);
        }}
        onClose={() => (termInstance = null)}
      />
    </div>
  {/if}
</div>
