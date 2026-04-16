<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  interface Props {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children }: Props = $props();
</script>

<div
  role="tablist"
  class={cn(
    'inline-flex h-9 items-center justify-start rounded-lg bg-muted p-1 text-muted-foreground',
    className
  )}
>
  {@render children?.()}
</div>
