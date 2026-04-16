<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  interface Props {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children }: Props = $props();
</script>

<kbd
  class={cn(
    'inline-flex h-5 min-w-5 items-center justify-center rounded border border-border bg-muted px-1.5 text-[10px] font-medium text-muted-foreground',
    className
  )}
>
  {@render children?.()}
</kbd>
