<script lang="ts">
  import { page } from '$app/stores';
  import { profile, region, aliasesPath, aliases } from '$lib/stores/aws';
  import { ipc } from '$lib/ipc';
  import { onMount } from 'svelte';
  import {
    Pulse,
    HardDrives,
    Stack,
    TerminalWindow,
    CaretRight,
    MagnifyingGlass,
    FolderOpen,
    Bell,
    GearSix,
    Sun,
    Moon,
    TextAa
  } from 'phosphor-svelte';

  const titles: Record<string, { icon: any; label: string }> = {
    '/':           { icon: Pulse,          label: 'Sessions' },
    '/instances':  { icon: HardDrives,     label: 'Instances' },
    '/containers': { icon: Stack,          label: 'Containers' },
    '/terminal':   { icon: TerminalWindow, label: 'Terminal' }
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

  // ─── Text size: cycles sm → md → lg → xl, persisted to localStorage ───
  type TextSize = 'sm' | 'md' | 'lg' | 'xl';
  const TEXT_SIZES: TextSize[] = ['sm', 'md', 'lg', 'xl'];
  const TEXT_SIZE_LABEL: Record<TextSize, string> = {
    sm: 'Small',
    md: 'Default',
    lg: 'Large',
    xl: 'Extra-large'
  };
  let textSize = $state<TextSize>('md');

  function applyTextSize(s: TextSize) {
    document.documentElement.setAttribute('data-text-size', s);
  }
  function cycleTextSize() {
    const next = TEXT_SIZES[(TEXT_SIZES.indexOf(textSize) + 1) % TEXT_SIZES.length];
    textSize = next;
    applyTextSize(next);
    try { localStorage.setItem('aws-tui:text-size', next); } catch { /* ignore */ }
  }
  onMount(() => {
    try {
      const saved = localStorage.getItem('aws-tui:text-size') as TextSize | null;
      if (saved && TEXT_SIZES.includes(saved)) {
        textSize = saved;
      }
    } catch { /* ignore */ }
    applyTextSize(textSize);
  });

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
    <span class="tui-breadcrumb-sep"><CaretRight size={11} weight="bold" /></span>
    <span class="tui-breadcrumb-current">
      <span class="tui-breadcrumb-current-icon"><CurrentIcon size={13} weight="bold" /></span>
      {current.label}
    </span>
  </div>

  <!-- Command palette trigger (placeholder — opens search later) -->
  <button type="button" class="tui-cmd-trigger" style="margin-left: 12px;" onclick={() => { /* TODO: command palette */ }}>
    <span class="tui-cmd-trigger-icon"><MagnifyingGlass size={13} weight="bold" /></span>
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
    <span style="display: inline-flex; color: var(--tui-fg-3);"><FolderOpen size={12} weight="regular" /></span>
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
      <Bell size={14} weight="regular" />
    </button>
    <button
      type="button"
      class="tui-iconbtn tui-iconbtn-md"
      title={`Text size: ${TEXT_SIZE_LABEL[textSize]} (click to cycle)`}
      aria-label="Cycle text size"
      onclick={cycleTextSize}
    >
      <TextAa size={14} weight="regular" />
    </button>
    <button
      type="button"
      class="tui-iconbtn tui-iconbtn-md"
      title="Settings"
      aria-label="Settings"
    >
      <GearSix size={14} weight="regular" />
    </button>
    <button
      type="button"
      class="tui-iconbtn tui-iconbtn-md"
      title="Toggle theme"
      aria-label="Toggle theme"
      onclick={toggleTheme}
    >
      {#if dark}
        <Sun size={14} weight="regular" />
      {:else}
        <Moon size={14} weight="regular" />
      {/if}
    </button>
  </div>
</header>
