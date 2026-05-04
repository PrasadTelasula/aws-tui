<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    open: boolean;
    onOpenChange?: (open: boolean) => void;
    children?: Snippet;
  }

  let { open, onOpenChange, children }: Props = $props();

  function close() {
    onOpenChange?.(false);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    role="presentation"
    class="fixed inset-0 z-50"
    onkeydown={onKeydown}
  >
    <!-- Overlay -->
    <div
      class="fixed inset-0 bg-black/60 backdrop-blur-sm"
      onclick={close}
      aria-hidden="true"
    ></div>
    <!-- Content portal -->
    {@render children?.()}
  </div>
{/if}
