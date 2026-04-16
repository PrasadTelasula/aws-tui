<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import PageHeader from '$lib/components/app-shell/page-header.svelte';
  import { Badge, Button } from '$lib/components/ui';
  import { TerminalSquare, X } from 'lucide-svelte';

  let container: HTMLDivElement;
  let term: any = null;
  let fit: any = null;

  onMount(async () => {
    const { Terminal } = await import('@xterm/xterm');
    const { FitAddon } = await import('@xterm/addon-fit');
    const { WebLinksAddon } = await import('@xterm/addon-web-links');

    term = new Terminal({
      fontFamily: 'var(--font-mono)',
      fontSize: 13,
      theme: {
        background: 'rgba(0,0,0,0)',
        foreground: '#d4d4d4',
        cursor: '#ed8936',
        cursorAccent: '#1a1a1a',
        selectionBackground: 'rgba(237, 137, 54, 0.3)'
      },
      cursorBlink: true,
      allowTransparency: true,
      scrollback: 5000
    });

    fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(container);
    fit.fit();

    term.writeln('\x1b[1;33mAWS TUI — embedded terminal\x1b[0m');
    term.writeln('PTY backend not yet wired. Type freely to test the UI.');
    term.writeln('');
    term.write('\x1b[1;32m$\x1b[0m ');

    let buf = '';
    term.onData((data: string) => {
      if (data === '\r') {
        term.writeln('');
        if (buf.trim()) term.writeln(`\x1b[90m(would run: ${buf})\x1b[0m`);
        buf = '';
        term.write('\x1b[1;32m$\x1b[0m ');
      } else if (data === '\u007f') {
        if (buf.length > 0) {
          buf = buf.slice(0, -1);
          term.write('\b \b');
        }
      } else {
        buf += data;
        term.write(data);
      }
    });

    const onResize = () => fit?.fit();
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  });

  onDestroy(() => {
    term?.dispose();
  });

  function clearTerm() {
    term?.clear();
  }
</script>

<div class="flex h-full flex-col gap-4">
  <PageHeader
    title="Terminal"
    subtitle="Embedded shell with AWS CLI autocomplete (PTY backend coming soon)."
  >
    {#snippet actions()}
      <Badge variant="warn">PTY backend TODO</Badge>
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
      <span class="font-mono">bash · default · us-east-1</span>
    </div>
    <div bind:this={container} class="min-h-0 flex-1 p-2"></div>
  </div>
</div>
