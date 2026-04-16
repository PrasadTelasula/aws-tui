<script lang="ts">
  import { onMount } from 'svelte';
  import { ipc } from '$lib/ipc';
  import type { CredentialInfo, SessionStatus } from '$lib/types';
  import { Badge, Button } from '$lib/components/ui';
  import { Check, Copy, Eye, EyeOff, KeyRound, X } from 'lucide-svelte';

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
      try {
        await navigator.clipboard.writeText(value);
      } catch {
        return;
      }
    }
    copiedField = field;
    setTimeout(() => {
      if (copiedField === field) copiedField = null;
    }, 1500);
  }

  function mask(value: string): string {
    if (!value) return '';
    if (revealed) return value;
    if (value.length <= 8) return '••••••';
    return value.slice(0, 4) + '…' + value.slice(-4);
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

<div
  role="dialog"
  aria-modal="true"
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4"
  onclick={onClose}
>
  <div
    class="w-full max-w-lg overflow-hidden rounded-lg border border-border bg-card shadow-xl"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="flex items-center justify-between border-b border-border px-5 py-3">
      <div class="flex items-center gap-2">
        <KeyRound class="h-4 w-4 text-primary" />
        <h2 class="font-mono text-sm font-semibold">{alias}</h2>
        {#if status?.identityAccount}
          <Badge variant="muted">acct {status.identityAccount}</Badge>
        {/if}
      </div>
      <button
        onclick={onClose}
        class="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
        aria-label="Close"
      >
        <X class="h-4 w-4" />
      </button>
    </div>

    <div class="px-5 py-4">
      {#if error}
        <p class="text-sm text-status-error">{error}</p>
      {:else if !creds}
        <p class="text-sm text-muted-foreground">Loading credentials…</p>
      {:else}
        {#if status?.identityArn}
          <p class="mb-4 break-all font-mono text-xs text-muted-foreground">
            {status.identityArn}
          </p>
        {/if}

        <div class="space-y-3 text-sm">
          {@render row('Access Key ID', creds.accessKeyId, 'akid')}
          {@render row('Secret Access Key', creds.secretAccessKey, 'secret', true)}
          {#if creds.sessionToken}
            {@render row('Session Token', creds.sessionToken, 'token', true)}
          {/if}
          {#if creds.expiration}
            <div class="flex items-center justify-between">
              <span class="text-xs text-muted-foreground">Expires</span>
              <span class="font-mono text-xs">{creds.expiration}</span>
            </div>
          {/if}
        </div>

        <div class="mt-5 flex items-center justify-between gap-2">
          <Button variant="ghost" size="sm" onclick={() => (revealed = !revealed)}>
            {#if revealed}
              <EyeOff class="h-3.5 w-3.5" /> Hide secrets
            {:else}
              <Eye class="h-3.5 w-3.5" /> Reveal secrets
            {/if}
          </Button>
          <Button size="sm" onclick={copyAllAsExport}>
            {#if copiedField === 'export'}
              <Check class="h-3.5 w-3.5" /> Copied
            {:else}
              <Copy class="h-3.5 w-3.5" /> Copy as export
            {/if}
          </Button>
        </div>
      {/if}
    </div>
  </div>
</div>

{#snippet row(label: string, value: string, field: string, secret = false)}
  <div class="flex items-center justify-between gap-3">
    <span class="shrink-0 text-xs text-muted-foreground">{label}</span>
    <code
      class="flex-1 truncate rounded bg-muted px-2 py-1 text-right font-mono text-xs"
      class:text-muted-foreground={secret && !revealed}
    >
      {secret ? mask(value) : value}
    </code>
    <button
      type="button"
      onclick={() => copy(value, field)}
      class="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
      aria-label="Copy"
    >
      {#if copiedField === field}
        <Check class="h-3.5 w-3.5 text-status-ok" />
      {:else}
        <Copy class="h-3.5 w-3.5" />
      {/if}
    </button>
  </div>
{/snippet}
