<script lang="ts">
  import { onMount } from 'svelte';
  import { ipc } from '$lib/ipc';
  import type { CredentialInfo, SessionStatus } from '$lib/types';
  import { Badge, Button, Dialog, DialogContent, DialogHeader, DialogTitle } from '$lib/components/ui';
  import { Check, Copy, Eye, EyeOff, KeyRound } from 'lucide-svelte';

  interface Props {
    alias: string;
    status: SessionStatus | undefined;
    onClose: () => void;
  }

  let { alias, status, onClose }: Props = $props();

  let creds = $state<CredentialInfo | null>(null);
  let revealed = $state(false);
  let copiedField = $state<string | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      creds = await ipc.getCredentials(alias);
      if (!creds) error = 'No credentials available for this session';
    } catch (e) {
      error = String(e);
    }
  });

  async function copy(value: string, field: string) {
    try {
      const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
      await writeText(value);
    } catch {
      try { await navigator.clipboard.writeText(value); } catch { return; }
    }
    copiedField = field;
    setTimeout(() => { if (copiedField === field) copiedField = null; }, 1500);
  }

  function mask(value: string): string {
    if (!value) return '';
    if (revealed) return value;
    if (value.length <= 8) return '••••••••';
    return value.slice(0, 4) + '••••••••' + value.slice(-4);
  }

  async function copyAllAsExport() {
    if (!creds) return;
    const lines = [
      `export AWS_ACCESS_KEY_ID=${creds.accessKeyId}`,
      `export AWS_SECRET_ACCESS_KEY=${creds.secretAccessKey}`
    ];
    if (creds.sessionToken) lines.push(`export AWS_SESSION_TOKEN=${creds.sessionToken}`);
    await copy(lines.join('\n'), 'export');
  }
</script>

<Dialog open={true} onOpenChange={(o) => { if (!o) onClose(); }}>
  <DialogContent onClose={onClose}>
    <DialogHeader>
      <DialogTitle>
        <div class="flex items-center gap-2 pr-8">
          <KeyRound class="h-4 w-4 text-primary" />
          <span class="font-mono">{alias}</span>
          {#if status?.identityAccount}
            <Badge variant="muted" class="text-[10px]">acct {status.identityAccount}</Badge>
          {/if}
        </div>
      </DialogTitle>
      {#if status?.identityArn}
        <p class="break-all font-mono text-[11px] text-muted-foreground">{status.identityArn}</p>
      {/if}
    </DialogHeader>

    <div class="px-6 py-4">
      {#if error}
        <p class="text-sm text-status-error">{error}</p>
      {:else if !creds}
        <div class="flex items-center gap-2 text-sm text-muted-foreground">
          <span class="h-3 w-3 animate-spin rounded-full border-2 border-primary border-t-transparent"></span>
          Loading credentials…
        </div>
      {:else}
        <div class="space-y-2.5 text-sm">
          {@render credRow('Access Key ID', creds.accessKeyId, 'akid')}
          {@render credRow('Secret Access Key', creds.secretAccessKey, 'secret', true)}
          {#if creds.sessionToken}
            {@render credRow('Session Token', creds.sessionToken, 'token', true)}
          {/if}
          {#if creds.expiration}
            <div class="flex items-center justify-between rounded-md bg-muted/40 px-3 py-2">
              <span class="text-xs text-muted-foreground">Expires</span>
              <span class="font-mono text-xs">{creds.expiration}</span>
            </div>
          {/if}
        </div>

        <div class="mt-5 flex items-center justify-between gap-2">
          <Button variant="ghost" size="sm" onclick={() => (revealed = !revealed)}>
            {#if revealed}
              <EyeOff class="h-3.5 w-3.5" /> Hide
            {:else}
              <Eye class="h-3.5 w-3.5" /> Reveal
            {/if}
          </Button>
          <Button size="sm" onclick={copyAllAsExport}>
            {#if copiedField === 'export'}
              <Check class="h-3.5 w-3.5 text-status-ok" /> Copied!
            {:else}
              <Copy class="h-3.5 w-3.5" /> Copy as export
            {/if}
          </Button>
        </div>
      {/if}
    </div>
  </DialogContent>
</Dialog>

{#snippet credRow(label: string, value: string, field: string, secret = false)}
  <div class="flex items-center gap-2 rounded-md bg-muted/40 px-3 py-2">
    <span class="w-28 shrink-0 text-[11px] text-muted-foreground">{label}</span>
    <code class="flex-1 truncate font-mono text-xs" class:text-muted-foreground={secret && !revealed}>
      {secret ? mask(value) : value}
    </code>
    <button
      type="button"
      onclick={() => copy(value, field)}
      class="shrink-0 rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    >
      {#if copiedField === field}
        <Check class="h-3.5 w-3.5 text-status-ok" />
      {:else}
        <Copy class="h-3.5 w-3.5" />
      {/if}
    </button>
  </div>
{/snippet}
