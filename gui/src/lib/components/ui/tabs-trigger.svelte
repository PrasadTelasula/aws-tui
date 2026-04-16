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
  let isActive = $derived($active === value);
</script>

<button
  role="tab"
  type="button"
  aria-selected={isActive}
  onclick={() => active.set(value)}
  class={cn(
    'inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium ring-offset-background transition-all',
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2',
    'disabled:pointer-events-none disabled:opacity-50',
    isActive
      ? 'bg-background text-foreground shadow'
      : 'text-muted-foreground hover:text-foreground',
    className
  )}
>
  {@render children?.()}
</button>
