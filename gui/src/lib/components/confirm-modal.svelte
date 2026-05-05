<script lang="ts">
  import { Button, Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '$lib/components/ui';
  import { Warning as AlertTriangle } from 'phosphor-svelte';

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

<Dialog open={true} onOpenChange={(o) => { if (!o) onCancel(); }}>
  <DialogContent size="sm" onClose={onCancel}>
    <DialogHeader>
      <DialogTitle>
        <div class="flex items-center gap-2 pr-8">
          {#if danger}
            <AlertTriangle class="h-4 w-4 text-status-warn" />
          {/if}
          {title}
        </div>
      </DialogTitle>
    </DialogHeader>
    <div class="px-6 py-4 text-sm text-muted-foreground">
      {message}
    </div>
    <DialogFooter>
      <Button variant="ghost" size="sm" onclick={onCancel}>{cancelLabel}</Button>
      <Button variant={danger ? 'destructive' : 'default'} size="sm" onclick={onConfirm}>
        {confirmLabel}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
