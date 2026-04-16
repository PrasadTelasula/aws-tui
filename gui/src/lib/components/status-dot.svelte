<script lang="ts">
  import { cn } from '$lib/utils';

  interface Props {
    tone: 'ok' | 'warn' | 'error' | 'info' | 'muted';
    class?: string;
    pulse?: boolean;
  }

  let { tone, class: className, pulse = false }: Props = $props();

  const toneMap: Record<Props['tone'], string> = {
    ok: 'bg-status-ok',
    warn: 'bg-status-warn',
    error: 'bg-status-error',
    info: 'bg-status-info',
    muted: 'bg-muted-foreground/50'
  };
</script>

<span class="relative inline-flex h-2 w-2 items-center justify-center">
  {#if pulse}
    <span
      class={cn('absolute h-full w-full animate-ping rounded-full opacity-60', toneMap[tone])}
    ></span>
  {/if}
  <span class={cn('relative h-2 w-2 rounded-full', toneMap[tone], className)}></span>
</span>
