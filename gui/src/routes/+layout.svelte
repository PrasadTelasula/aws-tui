<script lang="ts">
  import '../app.css';
  import Sidebar from '$lib/components/app-shell/sidebar.svelte';
  import Topbar from '$lib/components/app-shell/topbar.svelte';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();
</script>

<div class="flex h-full">
  <Sidebar />
  <div class="flex min-w-0 flex-1 flex-col">
    <Topbar />
    <main class="min-h-0 flex-1 overflow-hidden">
      {@render children()}
    </main>
  </div>
</div>
