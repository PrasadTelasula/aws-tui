<script lang="ts" module>
  import { getContext, setContext } from 'svelte';
  import { writable, type Writable } from 'svelte/store';

  const KEY = Symbol('tabs');

  export function createTabsContext(initial: string) {
    const value = writable(initial);
    setContext(KEY, value);
    return value;
  }

  export function useTabsContext(): Writable<string> {
    return getContext(KEY);
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  interface Props {
    value: string;
    class?: string;
    children?: Snippet;
    onValueChange?: (v: string) => void;
  }

  let { value = $bindable(), class: className, children, onValueChange }: Props = $props();

  const store = createTabsContext(value);
  store.subscribe((v) => {
    if (v !== value) {
      value = v;
      onValueChange?.(v);
    }
  });
  $effect(() => {
    store.set(value);
  });
</script>

<div class={cn('flex flex-col', className)}>
  {@render children?.()}
</div>
