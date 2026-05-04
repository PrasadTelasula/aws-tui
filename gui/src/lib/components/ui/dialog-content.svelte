<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';
  import { X } from 'lucide-svelte';

  interface Props {
    class?: string;
    children?: Snippet;
    onClose?: () => void;
    size?: 'sm' | 'md' | 'lg' | 'xl';
  }

  let { class: className, children, onClose, size = 'md' }: Props = $props();

  const sizeMap = {
    sm: 'max-w-sm',
    md: 'max-w-lg',
    lg: 'max-w-2xl',
    xl: 'max-w-4xl'
  };
</script>

<div
  class={cn(
    'fixed left-1/2 top-1/2 z-50 w-full -translate-x-1/2 -translate-y-1/2',
    'rounded-xl border border-border bg-card shadow-2xl',
    'flex flex-col overflow-hidden',
    sizeMap[size],
    className
  )}
  onclick={(e) => e.stopPropagation()}
  role="dialog"
  aria-modal="true"
>
  {#if onClose}
    <button
      onclick={onClose}
      class="absolute right-4 top-4 z-10 inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      aria-label="Close"
    >
      <X class="h-4 w-4" />
    </button>
  {/if}
  {@render children?.()}
</div>
