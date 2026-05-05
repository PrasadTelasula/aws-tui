<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ipc } from '$lib/ipc';
  import { X, PlugCharging } from 'phosphor-svelte';

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
      fontFamily: '"Geist Mono Variable", ui-monospace, monospace',
      fontSize: 12.5,
      lineHeight: 1.5,
      theme: {
        background: '#000000',
        foreground: '#E8EEF6',
        cursor: '#FFAC33',
        cursorAccent: '#000000',
        selectionBackground: 'rgba(255, 153, 0, 0.25)',
        black: '#18181B',
        brightBlack: '#71717A',
        red: '#F26B6B',
        brightRed: '#F58A8A',
        green: '#4ECB71',
        brightGreen: '#6FD58E',
        yellow: '#F2B544',
        brightYellow: '#FFC95E',
        blue: '#5AA9FF',
        brightBlue: '#7CBDFF',
        magenta: '#B07BFF',
        brightMagenta: '#C499FF',
        cyan: '#5BD0E0',
        brightCyan: '#7CDCE8',
        white: '#B8C4D6',
        brightWhite: '#E8EEF6'
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

<div class="flex h-full flex-col" style="background: #000000;">
  <!-- Header bar -->
  <div class="tui-pty-header">
    <span class="tui-pty-header-dots"><span></span><span></span><span></span></span>
    <span class="tui-pty-header-title">
      <PlugCharging size={13} weight="regular" />

      <span style="min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{title}</span>
    </span>

    {#if exited}
      <span class="tui-pty-header-status" style="color: var(--tui-fg-4);">session ended</span>
    {:else}
      <span class="tui-pty-header-status" style={connected ? '' : 'color: var(--tui-fg-4);'}>
        <span
          class={connected ? 'tui-dot tui-dot-ok' : 'tui-dot tui-dot-muted'}
          style="width: 5px; height: 5px;"
        ></span>
        {connected ? 'connected' : 'connecting…'}
      </span>
    {/if}

    {#if onClose}
      <button
        type="button"
        onclick={onClose}
        class="tui-iconbtn tui-iconbtn-sm"
        aria-label="Close terminal"
      >
        <X size={13} weight="bold" />
      </button>
    {/if}
  </div>

  <!-- Terminal canvas -->
  <div bind:this={container} class="tui-pty-body" style="padding: 8px 10px;"></div>
</div>
