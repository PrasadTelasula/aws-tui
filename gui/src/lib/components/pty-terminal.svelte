<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ipc } from '$lib/ipc';
  import { X, PlugZap } from 'lucide-svelte';

  interface Props {
    ptyId: string;
    title?: string;
    onReady: (rows: number, cols: number) => Promise<void>;
    onClose?: () => void;
  }

  let { ptyId, title = 'Terminal', onReady, onClose }: Props = $props();

  let container: HTMLDivElement;
  let term: any = null;
  let fit: any = null;
  let unlistenData: (() => void) | null = null;
  let unlistenExit: (() => void) | null = null;
  let connected = $state(false);
  let exited = $state(false);

  onMount(async () => {
    const { Terminal } = await import('@xterm/xterm');
    const { FitAddon } = await import('@xterm/addon-fit');
    const { WebLinksAddon } = await import('@xterm/addon-web-links');

    term = new Terminal({
      fontFamily: '"JetBrains Mono Variable", "JetBrains Mono", ui-monospace, monospace',
      fontSize: 12.5,
      lineHeight: 1.5,
      theme: {
        background: '#0d1117',
        foreground: '#e6edf3',
        cursor: '#e6edf3',
        cursorAccent: '#0d1117',
        selectionBackground: 'rgba(175, 127, 57, 0.3)',
        black: '#21262d',
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
      cursorStyle: 'bar',
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
        exited = true;
        term.writeln('\r\n\x1b[2m[session ended]\x1b[0m');
      });

      await onReady(term.rows, term.cols);
      connected = true;

      term.onData((data: string) => {
        if (connected) ipc.ptyWrite(ptyId, data).catch(() => {});
      });

      term.onResize(({ rows, cols }: { rows: number; cols: number }) => {
        if (connected) ipc.ptyResize(ptyId, rows, cols).catch(() => {});
      });
    } catch (e) {
      term.writeln(`\x1b[1;31m${String(e)}\x1b[0m`);
    }

    const onWindowResize = () => fit?.fit();
    window.addEventListener('resize', onWindowResize);
    return () => window.removeEventListener('resize', onWindowResize);
  });

  onDestroy(() => {
    unlistenData?.();
    unlistenExit?.();
    if (connected) ipc.ptyClose(ptyId).catch(() => {});
    term?.dispose();
  });
</script>

<div class="flex h-full flex-col bg-[#0d1117]">
  <!-- Header bar -->
  <div class="flex h-9 shrink-0 items-center gap-2.5 border-b border-white/5 bg-[#161b22] px-3">
    <PlugZap class="h-3.5 w-3.5 text-white/30" />

    <span class="min-w-0 flex-1 truncate font-mono text-[11px] text-white/40">{title}</span>

    <!-- Status indicator -->
    <div class="flex items-center gap-1.5">
      {#if exited}
        <span class="text-[10px] text-white/30">session ended</span>
      {:else}
        <span class={connected
          ? 'h-1.5 w-1.5 rounded-full bg-emerald-500 shadow-[0_0_6px_theme(colors.emerald.500)]'
          : 'h-1.5 w-1.5 rounded-full bg-white/20 animate-pulse'
        }></span>
        <span class="text-[10px] text-white/30">{connected ? 'connected' : 'connecting…'}</span>
      {/if}
    </div>

    {#if onClose}
      <button
        onclick={onClose}
        class="ml-1 rounded p-0.5 text-white/25 transition-colors hover:bg-white/5 hover:text-white/60"
        aria-label="Close terminal"
      >
        <X class="h-3.5 w-3.5" />
      </button>
    {/if}
  </div>

  <!-- Terminal canvas -->
  <div bind:this={container} class="min-h-0 flex-1 px-2 py-1"></div>
</div>
