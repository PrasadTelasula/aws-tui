<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { ipc } from '$lib/ipc';
  import { profile, region } from '$lib/stores/aws';
  import { Trash2, TerminalSquare, CircleDot } from 'lucide-svelte';

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
        term.writeln('\r\n\x1b[2m[process exited]\x1b[0m');
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

<div class="flex h-full flex-col bg-[#0d1117]">
  <!-- Terminal toolbar -->
  <div class="flex h-10 shrink-0 items-center gap-3 border-b border-white/5 bg-[#161b22] px-4">
    <TerminalSquare class="h-3.5 w-3.5 text-white/30" />
    <span class="text-xs font-medium text-white/50">Terminal</span>

    <!-- Profile · Region -->
    <div class="flex items-center gap-1.5">
      <span class="h-3 w-px bg-white/10"></span>
      <span class="font-mono text-[11px] text-white/30">{$profile}</span>
      <span class="text-white/20">·</span>
      <span class="font-mono text-[11px] text-white/30">{$region}</span>
    </div>

    <!-- Status dot -->
    <div class="ml-auto flex items-center gap-3">
      <div class="flex items-center gap-1.5">
        <span class={connected
          ? 'h-1.5 w-1.5 rounded-full bg-emerald-500 shadow-[0_0_6px_theme(colors.emerald.500)]'
          : 'h-1.5 w-1.5 rounded-full bg-white/20'
        }></span>
        <span class="text-[11px] text-white/40">{connected ? 'connected' : 'disconnected'}</span>
      </div>

      <!-- Clear button -->
      <button
        onclick={() => term?.clear()}
        class="flex items-center gap-1 rounded px-2 py-1 text-[11px] text-white/30 transition-colors hover:bg-white/5 hover:text-white/60"
      >
        <Trash2 class="h-3 w-3" />
        Clear
      </button>
    </div>
  </div>

  <!-- Terminal canvas -->
  <div class="min-h-0 flex-1">
    <div bind:this={container} class="h-full px-2 py-1.5"></div>
  </div>
</div>
