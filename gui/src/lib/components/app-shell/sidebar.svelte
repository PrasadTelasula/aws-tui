<script lang="ts">
  import { page } from '$app/stores';
  import { navEntries } from '$lib/nav';
  import { Cloud } from 'lucide-svelte';
  import { cn } from '$lib/utils';
</script>

<aside
  class="flex h-full w-60 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground"
>
  <div class="flex h-14 items-center gap-2 px-4 font-semibold">
    <Cloud class="h-5 w-5 text-primary" />
    <span class="tracking-tight">AWS TUI</span>
  </div>

  <nav class="flex-1 space-y-1 px-2 py-2 text-sm">
    {#each navEntries as entry (entry.href)}
      {@const isActive =
        entry.href === '/'
          ? $page.url.pathname === '/'
          : $page.url.pathname.startsWith(entry.href)}
      {@const Icon = entry.icon}
      <a
        href={entry.href}
        class={cn(
          'flex items-center gap-3 rounded-md px-3 py-2 font-medium transition-colors',
          isActive
            ? 'bg-sidebar-accent text-sidebar-accent-foreground'
            : 'text-sidebar-foreground/80 hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground'
        )}
      >
        <Icon class="h-4 w-4" />
        <span>{entry.label}</span>
      </a>
    {/each}
  </nav>

  <div class="border-t border-sidebar-border p-3 text-xs text-muted-foreground">
    <div class="flex items-center justify-between">
      <span class="font-medium">Version</span>
      <span class="font-mono">0.1.0</span>
    </div>
  </div>
</aside>
