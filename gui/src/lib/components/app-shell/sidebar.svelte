<script lang="ts">
  import { page } from '$app/stores';
  import { sidebarOpen } from '$lib/stores/ui';
  import { sessions } from '$lib/stores/aws';
  import { isActive } from '$lib/sessions-helpers';
  import {
    Pulse,
    HardDrives,
    Stack,
    TerminalWindow,
    SidebarSimple,
    GearSix
  } from 'phosphor-svelte';
  import StatusDot from '$lib/components/status-dot.svelte';

  const navItems = [
    { href: '/',           label: 'Sessions',   icon: Pulse,          shortcut: 'g s' },
    { href: '/instances',  label: 'Instances',  icon: HardDrives,     shortcut: 'g i' },
    { href: '/containers', label: 'Containers', icon: Stack,          shortcut: 'g c' },
    { href: '/terminal',   label: 'Terminal',   icon: TerminalWindow, shortcut: 'g t' },
    { href: '/settings',   label: 'Settings',   icon: GearSix,        shortcut: 'g ,' }
  ];

  function isCurrent(href: string): boolean {
    return href === '/' ? $page.url.pathname === '/' : $page.url.pathname.startsWith(href);
  }

  function toggle() { sidebarOpen.update((v) => !v); }

  // ⌘B / Ctrl+B keyboard shortcut + g s/i/c/t jump shortcuts
  let pendingG = false;
  let pendingTimer: ReturnType<typeof setTimeout> | null = null;
  function onKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'b') {
      e.preventDefault();
      toggle();
      return;
    }
    const tag = (e.target as HTMLElement | null)?.tagName ?? '';
    if (tag === 'INPUT' || tag === 'TEXTAREA') return;
    if (e.key === 'g' && !e.metaKey && !e.ctrlKey) {
      pendingG = true;
      if (pendingTimer) clearTimeout(pendingTimer);
      pendingTimer = setTimeout(() => { pendingG = false; }, 800);
      return;
    }
    if (pendingG) {
      const map: Record<string, string> = { s: '/', i: '/instances', c: '/containers', t: '/terminal' };
      const dest = map[e.key.toLowerCase()];
      if (dest) {
        e.preventDefault();
        window.location.href = dest;
      }
      pendingG = false;
      if (pendingTimer) clearTimeout(pendingTimer);
    }
  }

  let runningCount = $derived(Object.values($sessions).filter((s) => isActive(s)).length);
</script>

<svelte:window onkeydown={onKeydown} />

<aside class="tui-sidebar" class:is-collapsed={!$sidebarOpen}>
  <!-- Header / brand -->
  <div class="tui-sidebar-header">
    <div class="tui-logo" title="aws-tui">
      <span class="tui-logo-glyph">A</span>
    </div>
    <div class="tui-sidebar-title">
      <span class="tui-sidebar-title-name">aws-tui</span>
      <span class="tui-sidebar-title-sub">cloud manager</span>
    </div>
  </div>

  <!-- Navigate -->
  <div class="tui-sidebar-section">
    <div class="tui-sidebar-section-label">Navigate</div>
  </div>
  <nav style="padding: 0 8px; display: flex; flex-direction: column; gap: 2px;">
    {#each navItems as item (item.href)}
      {@const active = isCurrent(item.href)}
      {@const Icon = item.icon}
      {@const showCount = item.href === '/' && runningCount > 0}
      <a
        href={item.href}
        class="tui-nav-item"
        class:is-active={active}
        title={$sidebarOpen ? undefined : item.label}
      >
        <span class="tui-nav-item-icon"><Icon size={15} weight={active ? 'bold' : 'regular'} /></span>
        <span class="tui-nav-item-label">{item.label}</span>
        {#if showCount}
          <span class="tui-nav-item-badge">{runningCount}</span>
        {/if}
      </a>
    {/each}
  </nav>

  <!-- Footer / collapse toggle -->
  <div class="tui-sidebar-footer">
    {#if $sidebarOpen}
      <span class="tui-sidebar-footer-version">
        <StatusDot tone="ok" />
        <span>v0.1.0</span>
      </span>
    {/if}
    <button
      type="button"
      class="tui-iconbtn tui-iconbtn-sm"
      onclick={toggle}
      title={$sidebarOpen ? 'Collapse sidebar (⌘B)' : 'Expand sidebar (⌘B)'}
      aria-label="Toggle sidebar"
    >
      <SidebarSimple size={14} weight="regular" />

    </button>
  </div>
</aside>
