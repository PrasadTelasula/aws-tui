<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ipc } from '$lib/ipc';
  import { X } from 'lucide-svelte';

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
        term.writeln('\r\n\x1b[1;31m[session ended]\x1b[0m');
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

<div class="flex h-full flex-col rounded-lg border border-border bg-[#0f1114] shadow-inner">
  <div class="flex items-center justify-between border-b border-white/5 px-3 py-1.5">
    <span class="font-mono text-xs text-white/60">{title}</span>
    <div class="flex items-center gap-2">
      <span class="h-2 w-2 rounded-full {connected ? 'bg-green-500' : 'bg-white/20'}"></span>
      {#if onClose}
        <button
          onclick={onClose}
          class="rounded p-0.5 text-white/40 transition-colors hover:text-white/80"
          aria-label="Close terminal"
        >
          <X class="h-3.5 w-3.5" />
        </button>
      {/if}
    </div>
  </div>
  <div bind:this={container} class="min-h-0 flex-1 p-2"></div>
</div>
