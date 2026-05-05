<script lang="ts">
  import { page } from '$app/stores';
  import { profile, region, aliasesPath, aliases } from '$lib/stores/aws';
  import { ipc } from '$lib/ipc';
  import {
    Activity,
    Server,
    Boxes,
    TerminalSquare,
    ChevronRight,
    Search,
    FolderOpen,
    Bell,
    Settings,
    Sun,
    Moon
  } from 'lucide-svelte';

  const titles: Record<string, { icon: any; label: string }> = {
    '/':           { icon: Activity,        label: 'Sessions' },
    '/instances':  { icon: Server,          label: 'Instances' },
    '/containers': { icon: Boxes,           label: 'Containers' },
    '/terminal':   { icon: TerminalSquare,  label: 'Terminal' }
  };

  let current = $derived.by(() => {
    const p = $page.url.pathname;
    if (p === '/') return titles['/'];
    return titles[Object.keys(titles).find((k) => k !== '/' && p.startsWith(k)) ?? '/'];
  });

  let dark = $state(true);
  function toggleTheme() {
    dark = !dark;
    const root = document.documentElement;
    if (dark) {
      root.removeAttribute('data-theme');
      root.classList.add('dark');
    } else {
      root.setAttribute('data-theme', 'light');
      root.classList.remove('dark');
    }
  }

  // Inline-edit profile / region
  let editingProfile = $state(false);
  let editingRegion = $state(false);
  let profileEl = $state<HTMLInputElement | null>(null);
  let regionEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (editingProfile && profileEl) { profileEl.focus(); profileEl.select(); }
  });
  $effect(() => {
    if (editingRegion && regionEl) { regionEl.focus(); regionEl.select(); }
  });

  function commitEdit(e: KeyboardEvent, close: () => void) {
    if (e.key === 'Enter' || e.key === 'Escape') { e.preventDefault(); close(); }
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
  let CurrentIcon = $derived(current.icon);
</script>

<header class="tui-topbar">
  <!-- Breadcrumb -->
  <div class="tui-breadcrumb">
    <span class="tui-breadcrumb-org">aws-tui</span>
    <span class="tui-breadcrumb-sep"><ChevronRight size={11} /></span>
    <span class="tui-breadcrumb-current">
      <span class="tui-breadcrumb-current-icon"><CurrentIcon size={13} strokeWidth={1.8} /></span>
      {current.label}
    </span>
  </div>

  <!-- Command palette trigger (placeholder — opens search later) -->
  <button type="button" class="tui-cmd-trigger" style="margin-left: 12px;" onclick={() => { /* TODO: command palette */ }}>
    <span class="tui-cmd-trigger-icon"><Search size={13} strokeWidth={1.8} /></span>
    <span>Jump to alias, instance, command…</span>
    <kbd class="tui-kbd">⌘K</kbd>
  </button>

  <div class="tui-topbar-spacer"></div>

  <!-- Aliases file pill -->
  <button
    type="button"
    class="tui-context-pill"
    title={$aliasesPath ?? 'No aliases file loaded'}
    onclick={pickFile}
  >
    <span style="display: inline-flex; color: var(--tui-fg-3);"><FolderOpen size={12} /></span>
    <span class="tui-context-pill-value">{basename ?? 'Load aliases…'}</span>
  </button>

  <!-- Profile -->
  {#if editingProfile}
    <label class="tui-context-pill is-editing">
      <span class="tui-context-pill-label">profile</span>
      <input
        bind:this={profileEl}
        bind:value={$profile}
        onblur={() => (editingProfile = false)}
        onkeydown={(e) => commitEdit(e, () => (editingProfile = false))}
        class="tui-context-pill-input"
        spellcheck={false}
      />
    </label>
  {:else}
    <button
      type="button"
      class="tui-context-pill"
      onclick={() => (editingProfile = true)}
      title="Click to edit profile"
    >
      <span class="tui-context-pill-label">profile</span>
      <span class="tui-context-pill-value">{$profile}</span>
    </button>
  {/if}

  <!-- Region -->
  {#if editingRegion}
    <label class="tui-context-pill is-editing">
      <span class="tui-context-pill-label">region</span>
      <input
        bind:this={regionEl}
        bind:value={$region}
        onblur={() => (editingRegion = false)}
        onkeydown={(e) => commitEdit(e, () => (editingRegion = false))}
        class="tui-context-pill-input"
        style="width: 9ch;"
        spellcheck={false}
      />
    </label>
  {:else}
    <button
      type="button"
      class="tui-context-pill"
      onclick={() => (editingRegion = true)}
      title="Click to edit region"
    >
      <span class="tui-context-pill-label">region</span>
      <span class="tui-context-pill-value">{$region}</span>
    </button>
  {/if}

  <div class="tui-topbar-divider"></div>

  <div class="tui-topbar-right">
    <button
      type="button"
      class="tui-iconbtn tui-iconbtn-md"
      title="Notifications"
      aria-label="Notifications"
    >
      <Bell size={14} strokeWidth={1.7} />
    </button>
    <button
      type="button"
      class="tui-iconbtn tui-iconbtn-md"
      title="Settings"
      aria-label="Settings"
    >
      <Settings size={14} strokeWidth={1.7} />
    </button>
    <button
      type="button"
      class="tui-iconbtn tui-iconbtn-md"
      title="Toggle theme"
      aria-label="Toggle theme"
      onclick={toggleTheme}
    >
      {#if dark}
        <Sun size={14} strokeWidth={1.7} />
      {:else}
        <Moon size={14} strokeWidth={1.7} />
      {/if}
    </button>
  </div>
</header>
