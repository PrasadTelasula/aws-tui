<script lang="ts">
  import { page } from '$app/stores';
  import { navEntries } from '$lib/nav';
  import { Cloud } from 'lucide-svelte';
  import { cn } from '$lib/utils';
</script>

<aside class="flex h-full w-56 shrink-0 flex-col border-r border-sidebar-border bg-sidebar">
  <!-- Logo -->
  <div class="flex h-14 items-center gap-2.5 px-4">
    <div class="flex h-7 w-7 items-center justify-center rounded-lg bg-primary/10">
      <Cloud class="h-4 w-4 text-primary" />
    </div>
    <div>
      <p class="text-sm font-semibold leading-none tracking-tight text-sidebar-foreground">
        AWS TUI
      </p>
      <p class="mt-0.5 text-[10px] leading-none text-sidebar-foreground/50">Cloud Manager</p>
    </div>
  </div>

  <div class="mx-3 border-t border-sidebar-border/60"></div>

  <!-- Nav -->
  <nav class="flex-1 space-y-0.5 px-2 py-3">
    {#each navEntries as entry (entry.href)}
      {@const isActive =
        entry.href === '/'
          ? $page.url.pathname === '/'
          : $page.url.pathname.startsWith(entry.href)}
      {@const Icon = entry.icon}
      <a
        href={entry.href}
        class={cn(
          'group flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors relative',
          isActive
            ? 'bg-sidebar-accent text-sidebar-accent-foreground'
            : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'
        )}
      >
        {#if isActive}
          <span class="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-r-full bg-primary"></span>
        {/if}
        <Icon
          class={cn(
            'h-4 w-4 shrink-0 transition-colors',
            isActive ? 'text-primary' : 'text-sidebar-foreground/50 group-hover:text-sidebar-foreground/80'
          )}
        />
        <span class="truncate">{entry.label}</span>
      </a>
    {/each}
  </nav>

  <!-- Footer -->
  <div class="border-t border-sidebar-border/60 px-4 py-3">
    <div class="flex items-center justify-between text-[10px] text-sidebar-foreground/40">
      <span class="font-medium uppercase tracking-wider">Version</span>
      <span class="font-mono">0.1.0</span>
    </div>
  </div>
</aside>
