<script lang="ts">
  import { page } from '$app/stores';
  import { navEntries } from '$lib/nav';
  import { profile, region, aliasesPath, aliases } from '$lib/stores/aws';
  import { ipc } from '$lib/ipc';
  import { Input } from '$lib/components/ui';
  import { FolderOpen, Moon, Sun, ChevronRight } from 'lucide-svelte';

  let current = $derived(
    navEntries.find((e) =>
      e.href === '/' ? $page.url.pathname === '/' : $page.url.pathname.startsWith(e.href)
    )
  );

  let dark = $state(false);

  function toggleTheme() {
    dark = !dark;
    document.documentElement.classList.toggle('dark', dark);
  }

  async function pickFile() {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({
      multiple: false,
      directory: false,
      title: 'Select aliases file',
      filters: [
        { name: 'Shell aliases', extensions: ['sh', 'bash', 'zsh', 'aliases', 'txt'] },
        { name: 'All files', extensions: ['*'] }
      ]
    });
    if (typeof selected !== 'string') return;
    const resp = await ipc.setAliasesPath(selected);
    aliasesPath.set(resp.path);
    aliases.set(resp.aliases);
  }

  let basename = $derived($aliasesPath ? $aliasesPath.split(/[\\/]/).pop() : null);
</script>

<header
  class="app-chrome-drag flex h-12 shrink-0 items-center gap-3 border-b border-border bg-background/95 px-4 backdrop-blur"
>
  <!-- Breadcrumb -->
  <div class="flex items-center gap-1.5 text-sm">
    <span class="font-medium text-muted-foreground/60">AWS TUI</span>
    {#if current}
      <ChevronRight class="h-3.5 w-3.5 text-muted-foreground/40" />
      <span class="font-medium text-foreground">{current.label}</span>
    {/if}
  </div>

  <div class="app-chrome-no-drag ml-auto flex items-center gap-1.5">
    <!-- Aliases file picker -->
    <button
      type="button"
      onclick={pickFile}
      title={$aliasesPath ?? 'No aliases file loaded'}
      class="inline-flex h-7 items-center gap-1.5 rounded-md border border-border/60 bg-muted/40 px-2.5 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
    >
      <FolderOpen class="h-3 w-3" />
      <span class="font-mono">{basename ?? 'Aliases…'}</span>
    </button>

    <div class="mx-1 h-4 w-px bg-border"></div>

    <!-- Profile -->
    <div class="flex items-center gap-1.5">
      <span class="text-[10px] font-medium uppercase tracking-wider text-muted-foreground/60">Profile</span>
      <Input
        class="h-7 w-24 rounded-md border-border/60 bg-muted/40 px-2 font-mono text-[11px] focus-visible:ring-1"
        bind:value={$profile}
        spellcheck={false}
      />
    </div>

    <!-- Region -->
    <div class="flex items-center gap-1.5">
      <span class="text-[10px] font-medium uppercase tracking-wider text-muted-foreground/60">Region</span>
      <Input
        class="h-7 w-28 rounded-md border-border/60 bg-muted/40 px-2 font-mono text-[11px] focus-visible:ring-1"
        bind:value={$region}
        spellcheck={false}
      />
    </div>

    <div class="mx-1 h-4 w-px bg-border"></div>

    <!-- Theme toggle -->
    <button
      type="button"
      onclick={toggleTheme}
      aria-label="Toggle theme"
      class="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    >
      {#if dark}
        <Sun class="h-3.5 w-3.5" />
      {:else}
        <Moon class="h-3.5 w-3.5" />
      {/if}
    </button>
  </div>
</header>
