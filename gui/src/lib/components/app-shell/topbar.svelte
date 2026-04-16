<script lang="ts">
  import { page } from '$app/stores';
  import { navEntries } from '$lib/nav';
  import { profile, region } from '$lib/stores/aws';
  import { Badge, Input } from '$lib/components/ui';
  import { Moon, Sun } from 'lucide-svelte';

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
