<script lang="ts">
  import { page } from '$app/stores';
  import { navEntries } from '$lib/nav';
  import { profile, region, aliasesPath, aliases } from '$lib/stores/aws';
  import { ipc } from '$lib/ipc';
  import { FolderOpen, Moon, Sun, ChevronRight, Pencil, Check } from 'lucide-svelte';
  import { cn } from '$lib/utils';

  let current = $derived(
    navEntries.find((e) =>
      e.href === '/' ? $page.url.pathname === '/' : $page.url.pathname.startsWith(e.href)
    )
  );

  let dark = $state(false);

  // Inline-editable profile / region
  let editingProfile = $state(false);
  let editingRegion = $state(false);
  let profileEl = $state<HTMLInputElement | null>(null);
  let regionEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (editingProfile && profileEl) {
      profileEl.focus();
      profileEl.select();
    }
  });

  $effect(() => {
    if (editingRegion && regionEl) {
      regionEl.focus();
      regionEl.select();
    }
  });

  function commitEdit(e: KeyboardEvent, close: () => void) {
    if (e.key === 'Enter' || e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }

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
  class="app-chrome-drag flex h-11 shrink-0 items-center gap-2 border-b border-border bg-background/95 px-4 backdrop-blur"
>
  <!-- Breadcrumb -->
  <div class="flex items-center gap-1.5 text-sm">
    <span class="text-[12px] font-medium text-muted-foreground/50">AWS TUI</span>
    {#if current}
      <ChevronRight class="h-3 w-3 text-muted-foreground/30" />
      {@const Icon = current.icon}
      <Icon class="h-3.5 w-3.5 text-muted-foreground/60" />
      <span class="text-[13px] font-semibold tracking-tight">{current.label}</span>
    {/if}
  </div>

  <div class="app-chrome-no-drag ml-auto flex items-center gap-1">
    <!-- Aliases file picker -->
    <button
      type="button"
      onclick={pickFile}
      title={$aliasesPath ?? 'No aliases file loaded'}
      class="flex h-7 items-center gap-1.5 rounded-md px-2.5 text-[11px] text-muted-foreground/70 transition-colors hover:bg-accent hover:text-foreground"
    >
      <FolderOpen class="h-3.5 w-3.5" />
      <span class="font-mono">{basename ?? 'Load aliases…'}</span>
    </button>

    <div class="mx-1 h-3.5 w-px bg-border"></div>

    <!-- Profile pill / inline input -->
    {#if editingProfile}
      <label
        class="flex h-7 items-center gap-1.5 rounded-md border border-primary/40 bg-background px-2 ring-2 ring-primary/10"
      >
        <span class="text-[9px] font-semibold uppercase tracking-widest text-muted-foreground/50">
          profile
        </span>
        <input
          bind:this={profileEl}
          bind:value={$profile}
          onblur={() => (editingProfile = false)}
          onkeydown={(e) => commitEdit(e, () => (editingProfile = false))}
          class="w-20 bg-transparent font-mono text-[11px] font-medium text-foreground outline-none"
          spellcheck={false}
        />
        <Check class="h-3 w-3 text-primary/60" />
      </label>
    {:else}
      <button
        onclick={() => (editingProfile = true)}
        class="group flex h-7 items-center gap-1.5 rounded-md border border-transparent px-2.5 transition-colors hover:border-border hover:bg-muted/50"
        title="Click to edit profile"
      >
        <span class="text-[9px] font-semibold uppercase tracking-widest text-muted-foreground/40">
          profile
        </span>
        <span class="font-mono text-[11px] font-semibold">{$profile}</span>
        <Pencil
          class="h-2.5 w-2.5 text-transparent transition-colors group-hover:text-muted-foreground/40"
        />
      </button>
    {/if}

    <!-- Region pill / inline input -->
    {#if editingRegion}
      <label
        class="flex h-7 items-center gap-1.5 rounded-md border border-primary/40 bg-background px-2 ring-2 ring-primary/10"
      >
        <span class="text-[9px] font-semibold uppercase tracking-widest text-muted-foreground/50">
          region
        </span>
        <input
          bind:this={regionEl}
          bind:value={$region}
          onblur={() => (editingRegion = false)}
          onkeydown={(e) => commitEdit(e, () => (editingRegion = false))}
          class="w-24 bg-transparent font-mono text-[11px] font-medium text-foreground outline-none"
          spellcheck={false}
        />
        <Check class="h-3 w-3 text-primary/60" />
      </label>
    {:else}
      <button
        onclick={() => (editingRegion = true)}
        class="group flex h-7 items-center gap-1.5 rounded-md border border-transparent px-2.5 transition-colors hover:border-border hover:bg-muted/50"
        title="Click to edit region"
      >
        <span class="text-[9px] font-semibold uppercase tracking-widest text-muted-foreground/40">
          region
        </span>
        <span class="font-mono text-[11px] font-semibold">{$region}</span>
        <Pencil
          class="h-2.5 w-2.5 text-transparent transition-colors group-hover:text-muted-foreground/40"
        />
      </button>
    {/if}

    <div class="mx-1 h-3.5 w-px bg-border"></div>

    <!-- Theme toggle -->
    <button
      type="button"
      onclick={toggleTheme}
      aria-label="Toggle theme"
      class="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground/60 transition-colors hover:bg-accent hover:text-foreground"
    >
      {#if dark}
        <Sun class="h-3.5 w-3.5" />
      {:else}
        <Moon class="h-3.5 w-3.5" />
      {/if}
    </button>
  </div>
</header>
