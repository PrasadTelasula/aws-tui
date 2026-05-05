<script lang="ts">
  import { page } from '$app/stores';
  import { sidebarOpen } from '$lib/stores/ui';
  import { Cloud, Activity, Server, Boxes, TerminalSquare, PanelLeftClose, PanelLeftOpen, LogIn, GitBranch } from 'lucide-svelte';
  import { cn } from '$lib/utils';

  // ── Nav groups ────────────────────────────────────────────────────────────
  const groups = [
    {
      label: 'AWS',
      items: [
        { href: '/',           label: 'Sessions',   icon: Activity,      description: 'SSO, SSM & IAM sessions' },
        { href: '/instances',  label: 'Instances',  icon: Server,        description: 'EC2 instances' },
        { href: '/containers', label: 'Containers', icon: Boxes,         description: 'ECS clusters & tasks' },
      ]
    },
    {
      label: 'Tools',
      items: [
        { href: '/terminal',   label: 'Terminal',   icon: TerminalSquare, description: 'AWS CLI shell' },
      ]
    }
  ];

  function isActive(href: string): boolean {
    return href === '/' ? $page.url.pathname === '/' : $page.url.pathname.startsWith(href);
  }

  function toggle() { sidebarOpen.update(v => !v); }

  // ⌘B / Ctrl+B keyboard shortcut
  function onKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'b') {
      e.preventDefault();
      toggle();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<aside
  data-state={$sidebarOpen ? 'expanded' : 'collapsed'}
  class={cn(
    'group/sidebar relative flex h-full shrink-0 flex-col border-r border-sidebar-border bg-sidebar-background',
    'transition-[width] duration-200 ease-in-out overflow-hidden',
    $sidebarOpen ? 'w-[15rem]' : 'w-[3.25rem]'
  )}
>
  <!-- ── Header ─────────────────────────────────────────────────────────── -->
  <div class="flex h-[3.5rem] shrink-0 items-center gap-3 px-3">
    <!-- App icon -->
    <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/15 ring-1 ring-primary/20">
      <Cloud class="h-4 w-4 text-primary" />
    </div>

    <!-- App name — fades out when collapsed -->
    <div class={cn(
      'flex min-w-0 flex-1 flex-col overflow-hidden transition-all duration-200',
      $sidebarOpen ? 'opacity-100 w-auto' : 'opacity-0 w-0'
    )}>
      <span class="truncate text-[13px] font-semibold tracking-tight text-sidebar-foreground">
        AWS TUI
      </span>
      <span class="truncate text-[10px] text-sidebar-foreground/40">Cloud Manager</span>
    </div>
  </div>

  <div class="mx-3 h-px shrink-0 bg-sidebar-border/60"></div>

  <!-- ── Navigation ─────────────────────────────────────────────────────── -->
  <nav class="flex-1 overflow-y-auto overflow-x-hidden px-2 py-3">
    {#each groups as group, gi (group.label)}
      <!-- Group label -->
      <div class={cn(
        'mb-1 overflow-hidden transition-all duration-200',
        gi > 0 ? 'mt-3' : '',
        $sidebarOpen ? 'h-5 opacity-100' : 'h-0 opacity-0'
      )}>
        <p class="px-2 text-[10px] font-semibold uppercase tracking-widest text-sidebar-foreground/40">
          {group.label}
        </p>
      </div>

      <!-- Menu items -->
      {#each group.items as item (item.href)}
        {@const active = isActive(item.href)}
        {@const Icon = item.icon}
        <a
          href={item.href}
          title={$sidebarOpen ? undefined : item.label}
          aria-label={item.label}
          class={cn(
            'group/item relative mb-0.5 flex h-9 items-center gap-3 rounded-md px-2 text-sm font-medium outline-none transition-colors',
            'focus-visible:ring-2 focus-visible:ring-ring',
            active
              ? 'bg-sidebar-accent text-sidebar-accent-foreground'
              : 'text-sidebar-foreground/60 hover:bg-sidebar-accent/60 hover:text-sidebar-foreground'
          )}
        >
          <!-- Active indicator bar -->
          {#if active}
            <span class="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-r-full bg-primary"></span>
          {/if}

          <!-- Icon -->
          <Icon
            class={cn(
              'h-[1.05rem] w-[1.05rem] shrink-0 transition-colors',
              active
                ? 'text-primary'
                : 'text-sidebar-foreground/40 group-hover/item:text-sidebar-foreground/70'
            )}
          />

          <!-- Label — fades out when collapsed -->
          <span class={cn(
            'flex-1 truncate transition-all duration-200',
            $sidebarOpen ? 'opacity-100' : 'opacity-0 w-0'
          )}>
            {item.label}
          </span>
        </a>
      {/each}
    {/each}
  </nav>

  <!-- ── Footer ─────────────────────────────────────────────────────────── -->
  <div class="shrink-0 border-t border-sidebar-border/60 p-2">
    <!-- Version row — only when expanded -->
    <div class={cn(
      'overflow-hidden transition-all duration-200',
      $sidebarOpen ? 'h-7 opacity-100 mb-1' : 'h-0 opacity-0 mb-0'
    )}>
      <div class="flex items-center justify-between px-2 py-1 text-[10px] text-sidebar-foreground/35">
        <span class="font-semibold uppercase tracking-wider">v0.1.0</span>
        <div class="flex items-center gap-1">
          <GitBranch class="h-2.5 w-2.5" />
          <span class="font-mono">main</span>
        </div>
      </div>
    </div>

    <!-- Collapse toggle button -->
    <button
      onclick={toggle}
      title={$sidebarOpen ? 'Collapse sidebar (⌘B)' : 'Expand sidebar (⌘B)'}
      class={cn(
        'flex h-9 w-full items-center rounded-md px-2 text-sidebar-foreground/40',
        'transition-colors hover:bg-sidebar-accent/60 hover:text-sidebar-foreground',
        !$sidebarOpen && 'justify-center'
      )}
    >
      {#if $sidebarOpen}
        <PanelLeftClose class="h-4 w-4 shrink-0" />
        <span class="ml-3 text-[13px] font-medium">Collapse</span>
      {:else}
        <PanelLeftOpen class="h-4 w-4 shrink-0" />
      {/if}
    </button>
  </div>

  <!-- ── Rail (hover strip on right edge to toggle) ─────────────────────── -->
  <button
    onclick={toggle}
    title={$sidebarOpen ? 'Collapse' : 'Expand'}
    aria-label="Toggle sidebar"
    class={cn(
      'absolute inset-y-0 right-0 w-1 cursor-col-resize opacity-0 transition-opacity',
      'hover:opacity-100 hover:bg-primary/20',
      'after:absolute after:inset-y-4 after:right-0 after:w-px after:bg-sidebar-border/80'
    )}
  ></button>
</aside>
