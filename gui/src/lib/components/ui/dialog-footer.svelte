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
  class={cn(
    'flex items-center justify-end gap-2 border-t border-border px-6 py-4',
    className
  )}
>
  {@render children?.()}
</div>
