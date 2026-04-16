<script lang="ts">
  import { Button } from '$lib/components/ui';
  import { AlertTriangle, X } from 'lucide-svelte';

  interface Props {
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    danger?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  }

  let {
    title,
    message,
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    danger = false,
    onConfirm,
    onCancel
  }: Props = $props();
</script>

<div
  role="dialog"
  aria-modal="true"
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
  onclick={onCancel}
>
  <div
    class="w-full max-w-sm overflow-hidden rounded-lg border border-border bg-card shadow-xl"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="flex items-center justify-between border-b border-border px-5 py-3">
      <div class="flex items-center gap-2">
        {#if danger}
          <AlertTriangle class="h-4 w-4 text-status-warn" />
        {/if}
        <h2 class="text-sm font-semibold">{title}</h2>
      </div>
      <button
        onclick={onCancel}
        class="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
        aria-label="Close"
      >
        <X class="h-4 w-4" />
      </button>
    </div>
    <div class="px-5 py-4 text-sm text-muted-foreground">{message}</div>
    <div class="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
      <Button variant="ghost" size="sm" onclick={onCancel}>{cancelLabel}</Button>
      <Button variant={danger ? 'destructive' : 'default'} size="sm" onclick={onConfirm}>
        {confirmLabel}
      </Button>
    </div>
  </div>
</div>
