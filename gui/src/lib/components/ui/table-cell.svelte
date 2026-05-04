<script lang="ts">
  import type { HTMLTdAttributes } from 'svelte/elements';
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  interface Props extends HTMLTdAttributes {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children, ...rest }: Props = $props();
</script>

<td class={cn('px-3 py-2.5 align-middle', className)} {...rest}>
  {@render children?.()}
</td>
