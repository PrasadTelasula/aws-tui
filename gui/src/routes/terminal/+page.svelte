<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { Badge, Button } from '$lib/components/ui';
  import { TerminalSquare, Trash2 } from 'lucide-svelte';
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
        background: '#0d1117',
        foreground: '#e6edf3',
        cursor: '#f78166',
        cursorAccent: '#0d1117',
        selectionBackground: 'rgba(247, 129, 102, 0.25)',
        black: '#0d1117',
        brightBlack: '#6e7681',
        red: '#ff7b72',
        brightRed: '#ffa198',
        green: '#3fb950',
        brightGreen: '#56d364',
        yellow: '#d29922',
        brightYellow: '#e3b341',
        blue: '#58a6ff',
        brightBlue: '#79c0ff',
        magenta: '#bc8cff',
        brightMagenta: '#d2a8ff',
        cyan: '#39c5cf',
        brightCyan: '#56d4dd',
        white: '#b1bac4',
        brightWhite: '#f0f6fc'
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
</script>

<div class="flex h-full flex-col">
  <!-- Toolbar -->
  <div class="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card/40 px-4">
    <div class="flex items-center gap-2">
      <TerminalSquare class="h-4 w-4 text-muted-foreground" />
      <h1 class="text-sm font-semibold">Terminal</h1>
    </div>
    <div class="flex items-center gap-2">
      <span class="font-mono text-[11px] text-muted-foreground">{$profile}</span>
      <span class="text-muted-foreground/40">·</span>
      <span class="font-mono text-[11px] text-muted-foreground">{$region}</span>
    </div>

    <div class="ml-auto flex items-center gap-2">
      <Badge variant={connected ? 'ok' : 'muted'} class="text-[10px]">
        {connected ? 'connected' : 'disconnected'}
      </Badge>
      <Button variant="ghost" size="sm" class="h-7 px-2" onclick={() => term?.clear()}>
        <Trash2 class="h-3.5 w-3.5" />
        Clear
      </Button>
    </div>
  </div>

  <!-- Terminal -->
  <div class="min-h-0 flex-1 bg-[#0d1117]">
    <div bind:this={container} class="h-full p-2"></div>
  </div>
</div>
