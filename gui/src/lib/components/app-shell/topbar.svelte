<script lang="ts">
  import { page } from '$app/stores';
  import { navEntries } from '$lib/nav';
  import { profile, region, aliasesPath, aliases } from '$lib/stores/aws';
  import { ipc } from '$lib/ipc';
  import { Badge, Input } from '$lib/components/ui';
  import { FolderOpen, Moon, Sun } from 'lucide-svelte';

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
  class="app-chrome-drag flex h-14 shrink-0 items-center justify-between gap-4 border-b border-border bg-background/80 px-6 backdrop-blur"
>
  <div class="flex flex-col">
    <h1 class="text-base font-semibold leading-none tracking-tight">
      {current?.label ?? 'AWS TUI'}
    </h1>
    {#if current}
      <p class="mt-1 text-xs text-muted-foreground">{current.description}</p>
    {/if}
  </div>

  <div class="app-chrome-no-drag flex items-center gap-3">
    <button
      type="button"
      onclick={pickFile}
      title={$aliasesPath ?? 'No aliases file loaded'}
      class="inline-flex h-8 items-center gap-2 rounded-md border border-border bg-background px-2.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    >
      <FolderOpen class="h-3.5 w-3.5" />
      <span class="font-mono">{basename ?? 'Load aliases…'}</span>
    </button>

    <div class="flex items-center gap-2 text-xs">
      <Badge variant="muted">Profile</Badge>
      <Input class="h-8 w-28 font-mono text-xs" bind:value={$profile} spellcheck={false} />
    </div>
    <div class="flex items-center gap-2 text-xs">
      <Badge variant="muted">Region</Badge>
      <Input class="h-8 w-32 font-mono text-xs" bind:value={$region} spellcheck={false} />
    </div>
    <button
      type="button"
      onclick={toggleTheme}
      aria-label="Toggle theme"
      class="inline-flex h-8 w-8 items-center justify-center rounded-md border border-border text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    >
      {#if dark}
        <Sun class="h-4 w-4" />
      {:else}
        <Moon class="h-4 w-4" />
      {/if}
    </button>
  </div>
</header>
