<script lang="ts">
  import type { Snippet } from 'svelte';
  import { useTabsContext } from './tabs.svelte';
  import { cn } from '$lib/utils';

  interface Props {
    value: string;
    class?: string;
    children?: Snippet;
  }

  let { value, class: className, children }: Props = $props();

  const active = useTabsContext();
</script>

{#if $active === value}
  <div role="tabpanel" class={cn('mt-2 focus-visible:outline-none', className)}>
    {@render children?.()}
  </div>
{/if}
