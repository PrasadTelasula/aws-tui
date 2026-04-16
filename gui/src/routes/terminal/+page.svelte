<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import PageHeader from '$lib/components/app-shell/page-header.svelte';
  import { Badge, Button } from '$lib/components/ui';
  import { TerminalSquare, X } from 'lucide-svelte';
  import { ipc } from '$lib/ipc';
  import { profile, region } from '$lib/stores/aws';

  let container: HTMLDivElement;
  let term: any = null;
  let fit: any = null;
  let unlistenData: (() => void) | null = null;
  let unlistenExit: (() => void) | null = null;
  let connected = $state(false);
  let error = $state<string | null>(null);

  const ptyId = `term-${Math.random().toString(36).slice(2, 10)}`;

  onMount(async () => {
    const { Terminal } = await import('@xterm/xterm');
    const { FitAddon } = await import('@xterm/addon-fit');
    const { WebLinksAddon } = await import('@xterm/addon-web-links');

    term = new Terminal({
      fontFamily: 'var(--font-mono)',
      fontSize: 13,
      theme: {
        background: '#0f1114',
        foreground: '#d4d4d4',
        cursor: '#ed8936',
        cursorAccent: '#0f1114',
        selectionBackground: 'rgba(237, 137, 54, 0.3)'
      },
      cursorBlink: true,
      scrollback: 10000,
      convertEol: true
    });

    fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(container);
    fit.fit();

    try {
      unlistenData = await ipc.onPtyData(ptyId, (chunk) => term.write(chunk));
      unlistenExit = await ipc.onPtyExit(ptyId, () => {
        connected = false;
        term.writeln('\r\n\x1b[1;31m[process exited]\x1b[0m');
      });

      await ipc.ptyOpen(ptyId, {
        rows: term.rows,
        cols: term.cols,
        profile: get(profile),
        region: get(region)
      });
      connected = true;

      term.onData((data: string) => {
        if (connected) ipc.ptyWrite(ptyId, data).catch(() => {});
      });

      term.onResize(({ rows, cols }: { rows: number; cols: number }) => {
        if (connected) ipc.ptyResize(ptyId, rows, cols).catch(() => {});
      });
    } catch (e) {
      error = String(e);
      term.writeln(`\x1b[1;31m${error}\x1b[0m`);
    }

    const onResize = () => fit?.fit();
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  });

  onDestroy(() => {
    unlistenData?.();
    unlistenExit?.();
    if (connected) ipc.ptyClose(ptyId).catch(() => {});
    term?.dispose();
  });

  function clearTerm() {
    term?.clear();
  }
</script>

<div class="flex h-full flex-col gap-4 px-6 py-5">
  <PageHeader
    title="Terminal"
    subtitle="Embedded shell with AWS_PROFILE and AWS_REGION pre-set."
  >
    {#snippet actions()}
      <Badge variant={connected ? 'ok' : 'muted'}>
        {connected ? 'connected' : 'disconnected'}
      </Badge>
      <Button variant="outline" size="sm" onclick={clearTerm}>
        <X class="h-3.5 w-3.5" /> Clear
      </Button>
    {/snippet}
  </PageHeader>

  <div
    class="flex min-h-0 flex-1 flex-col rounded-lg border border-border bg-[#0f1114] p-2 shadow-inner"
  >
    <div class="flex items-center gap-2 border-b border-white/5 px-2 py-1.5 text-xs text-white/60">
      <TerminalSquare class="h-3.5 w-3.5" />
      <span class="font-mono">pty · {$profile} · {$region}</span>
    </div>
    <div bind:this={container} class="min-h-0 flex-1 p-2"></div>
  </div>
</div>
