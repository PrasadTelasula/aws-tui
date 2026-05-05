<script lang="ts">
  import { page } from '$app/stores';
  import { profile, region, aliasesPath, aliases, sessions } from '$lib/stores/aws';
  import { isActive } from '$lib/sessions-helpers';
  import { ipc } from '$lib/ipc';
  import { onMount } from 'svelte';
  import { clickOutside } from '$lib/utils';
  import StatusDot from '$lib/components/status-dot.svelte';
  import {
    Pulse,
    HardDrives,
    Stack,
    TerminalWindow,
    CaretRight,
    CaretDown,
    Check,
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

  // ─── Profile / Region dropdowns ─────────────────────────────────────
  let profileMenuOpen = $state(false);
  let regionMenuOpen = $state(false);

  // A profile is "active" if any alias bound to it has a live session.
  let activeProfiles = $derived.by(() => {
    const out = new Set<string>();
    for (const a of $aliases) {
      if (!a.profile) continue;
      const st = $sessions[a.name];
      if (st && isActive(st)) out.add(a.profile);
    }
    return out;
  });

  // Profiles to show in the picker: distinct profiles defined in the loaded
  // aliases file, plus the current $profile so the user never gets stuck.
  let profileOptions = $derived.by(() => {
    const set = new Set<string>();
    for (const a of $aliases) if (a.profile) set.add(a.profile);
    if ($profile) set.add($profile);
    return [...set].sort();
  });

  // A reasonable set of AWS commercial regions, plus any seen in aliases.
  const COMMON_REGIONS = [
    'us-east-1', 'us-east-2', 'us-west-1', 'us-west-2',
    'ca-central-1', 'sa-east-1',
    'eu-west-1', 'eu-west-2', 'eu-west-3', 'eu-central-1', 'eu-north-1',
    'ap-northeast-1', 'ap-northeast-2', 'ap-southeast-1', 'ap-southeast-2',
    'ap-south-1'
  ];
  let regionOptions = $derived.by(() => {
    const set = new Set<string>(COMMON_REGIONS);
    for (const a of $aliases) if (a.region) set.add(a.region);
    if ($region) set.add($region);
    return [...set].sort();
  });

  function selectProfile(p: string) {
    profile.set(p);
    profileMenuOpen = false;
  }
  function selectRegion(r: string) {
    region.set(r);
    regionMenuOpen = false;
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

  <!-- Profile dropdown -->
  <div class="tui-context-pill-wrap" use:clickOutside={() => (profileMenuOpen = false)}>
    <button
      type="button"
      class="tui-context-pill"
      class:is-editing={profileMenuOpen}
      onclick={() => (profileMenuOpen = !profileMenuOpen)}
      title="Switch profile"
    >
      <span class="tui-context-pill-label">profile</span>
      <span class="tui-context-pill-value">{$profile}</span>
      <span class="tui-context-pill-caret"><CaretDown size={10} weight="bold" /></span>
    </button>
    {#if profileMenuOpen}
      <div class="tui-context-menu" role="listbox">
        {#if profileOptions.length === 0}
          <div class="tui-context-menu-empty">No profiles found in aliases file</div>
        {:else}
          <div class="tui-context-menu-section">
            {activeProfiles.size} active · {profileOptions.length} total
          </div>
          {#each profileOptions as p (p)}
            {@const active = activeProfiles.has(p)}
            {@const selected = $profile === p}
            <button
              type="button"
              class="tui-context-menu-item"
              class:is-active={selected}
              onclick={() => selectProfile(p)}
            >
              <StatusDot tone={active ? 'ok' : 'muted'} pulse={active} size={6} />
              <span class="tui-context-menu-item-name">{p}</span>
              {#if selected}
                <span class="tui-context-menu-item-check"><Check size={11} weight="bold" /></span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    {/if}
  </div>

  <!-- Region dropdown -->
  <div class="tui-context-pill-wrap" use:clickOutside={() => (regionMenuOpen = false)}>
    <button
      type="button"
      class="tui-context-pill"
      class:is-editing={regionMenuOpen}
      onclick={() => (regionMenuOpen = !regionMenuOpen)}
      title="Switch region"
    >
      <span class="tui-context-pill-label">region</span>
      <span class="tui-context-pill-value">{$region}</span>
      <span class="tui-context-pill-caret"><CaretDown size={10} weight="bold" /></span>
    </button>
    {#if regionMenuOpen}
      <div class="tui-context-menu" role="listbox">
        {#each regionOptions as r (r)}
          {@const selected = $region === r}
          <button
            type="button"
            class="tui-context-menu-item"
            class:is-active={selected}
            onclick={() => selectRegion(r)}
          >
            <span class="tui-context-menu-item-name">{r}</span>
            {#if selected}
              <span class="tui-context-menu-item-check"><Check size={11} weight="bold" /></span>
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>

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
