<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  interface Props extends HTMLAttributes<HTMLTableRowElement> {
    class?: string;
    selected?: boolean;
    children?: Snippet;
  }

  let { class: className, selected = false, children, ...rest }: Props = $props();
</script>

<tr
  class={cn(
    'border-b transition-colors hover:bg-muted/50',
    selected && 'bg-muted/70 hover:bg-muted/70',
    className
  )}
  data-state={selected ? 'selected' : undefined}
  {...rest}
>
  {@render children?.()}
</tr>
