<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/app-shell/sidebar.svelte';
  import Topbar from '$lib/components/app-shell/topbar.svelte';
  import type { Snippet } from 'svelte';
  import { sidebarOpen } from '$lib/stores/ui';

  let { children }: { children: Snippet } = $props();

  // Default to dark theme on first paint. The topbar toggle flips between
  // 'dark' (no data-theme attribute) and 'light' (data-theme="light").
  onMount(() => {
    const root = document.documentElement;
    // We treat absence of `data-theme` as dark, matching the redesign tokens.
    if (!root.dataset.theme) {
      root.classList.add('dark'); // keeps legacy Tailwind components happy
    }
  });
</script>

<div class="tui-app" class:is-collapsed={!$sidebarOpen}>
  <Sidebar />
  <main class="tui-main">
    <Topbar />
    <div class="tui-route flex min-h-0 flex-1 flex-col overflow-hidden">
      {@render children()}
    </div>
  </main>
</div>

<style>
  .tui-route {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
</style>
