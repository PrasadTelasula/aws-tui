<script lang="ts">
  import type { HTMLThAttributes } from 'svelte/elements';
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  interface Props extends HTMLThAttributes {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children, ...rest }: Props = $props();
</script>

<th
  class={cn(
    'h-10 px-3 text-left align-middle text-xs font-medium text-muted-foreground',
    className
  )}
  {...rest}
>
  {@render children?.()}
</th>
