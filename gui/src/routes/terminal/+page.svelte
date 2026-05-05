<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { ipc } from '$lib/ipc';
  import { profile, region } from '$lib/stores/aws';
  import StatusDot from '$lib/components/status-dot.svelte';
  import { TerminalSquare, Trash2, MapPin } from 'lucide-svelte';

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
      fontFamily: '"Geist Mono Variable", ui-monospace, monospace',
      fontSize: 13,
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

<div class="tui-screen">
  <div class="tui-term-page">
    <div class="tui-term-window">
      <!-- Toolbar -->
      <div class="tui-term-toolbar">
        <span style="display: inline-flex; color: var(--tui-fg-3);">
          <TerminalSquare size={13} strokeWidth={1.7} />
        </span>
        <span style="font-weight: 600; color: var(--tui-fg-2); font-size: 12px;">Terminal</span>
        <span class="tui-term-toolbar-meta">
          <span style="color: var(--tui-fg-4);">·</span>
          <strong style="color: var(--tui-fg-2); font-weight: 600;">{$profile}</strong>
          <span style="color: var(--tui-fg-4);">·</span>
          <strong style="color: var(--tui-fg-2); font-weight: 600;">{$region}</strong>
        </span>
        <div class="tui-term-toolbar-spacer"></div>
        <span class={`tui-term-toolbar-status ${connected ? '' : 'is-off'}`}>
          <StatusDot tone={connected ? 'ok' : 'muted'} size={5} pulse={connected} />
          {connected ? 'connected' : 'disconnected'}
        </span>
        <button
          type="button"
          class="tui-btn tui-btn-ghost tui-btn-sm"
          onclick={() => term?.clear()}
        >
          <Trash2 size={11} strokeWidth={1.8} />
          Clear
        </button>
      </div>

      <!-- Canvas -->
      <div class="tui-term-canvas">
        <div bind:this={container} class="tui-term-canvas-inner"></div>
      </div>

      <!-- Status bar -->
      <div class="tui-term-statusbar">
        <span class="tui-term-statusbar-cell">
          <StatusDot tone="ok" size={5} />
          <strong>{$profile}</strong>
        </span>
        <span style="color: var(--tui-fg-4);">·</span>
        <span class="tui-term-statusbar-cell">
          <MapPin size={10} />
          <strong>{$region}</strong>
        </span>
        <span class="tui-term-statusbar-spacer"></span>
        <span class="tui-term-statusbar-cell">
          <kbd class="tui-kbd">↑</kbd> history · <kbd class="tui-kbd">tab</kbd> complete
        </span>
      </div>
    </div>
  </div>
</div>
