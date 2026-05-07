<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/app-shell/sidebar.svelte';
  import Topbar from '$lib/components/app-shell/topbar.svelte';
  import type { Snippet } from 'svelte';
  import { sidebarOpen } from '$lib/stores/ui';
  import { awsProfiles, awsConfigPath } from '$lib/stores/aws';
  import { ipc } from '$lib/ipc';

  let { children }: { children: Snippet } = $props();

  // Default to dark theme on first paint. The topbar toggle flips between
  // 'dark' (no data-theme attribute) and 'light' (data-theme="light").
  onMount(async () => {
    const root = document.documentElement;
    if (!root.dataset.theme) {
      root.classList.add('dark');
    }
    // Load AWS profiles once on app start so the topbar dropdown can
    // show every profile from ~/.aws/config — not only the active ones.
    try {
      const snap = await ipc.listAwsProfiles();
      awsProfiles.set(snap.profiles);
      awsConfigPath.set(snap.configPath);
    } catch (e) {
      console.warn('listAwsProfiles failed:', e);
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
