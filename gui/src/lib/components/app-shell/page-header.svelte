<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  interface Props {
    title: string;
    subtitle?: string;
    class?: string;
    actions?: Snippet;
  }

  let { title, subtitle, class: className, actions }: Props = $props();
</script>

<div class={cn('flex items-start justify-between gap-4', className)}>
  <div>
    <h2 class="text-xl font-semibold tracking-tight">{title}</h2>
    {#if subtitle}
      <p class="mt-1 text-sm text-muted-foreground">{subtitle}</p>
    {/if}
  </div>
  {#if actions}
    <div class="flex items-center gap-2">
      {@render actions()}
    </div>
  {/if}
</div>
