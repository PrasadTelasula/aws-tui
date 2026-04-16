<script lang="ts">
  import { tick } from 'svelte';
  import {
    Copy,
    KeyRound,
    Play,
    PowerOff,
    Square,
    Terminal as TermIcon
  } from 'lucide-svelte';
  import { Badge, Button } from '$lib/components/ui';
  import StatusDot from '$lib/components/status-dot.svelte';
  import {
    isActive,
    kindBadgeVariant,
    kindLabel,
    outputLineClass,
    portHint,
    stateLabel,
    stateTone
  } from '$lib/sessions-helpers';
  import { uptimeFrom, formatDuration } from '$lib/utils';
  import type { Alias, SessionStatus } from '$lib/types';

  interface Props {
    alias: Alias | null;
    status: SessionStatus | undefined;
    output: string[];
    /** Re-rendered on each tick so uptime updates. */
    nowTick: number;
    onStart: (a: Alias) => void;
    onStop: (a: Alias) => void;
    onShowCredentials: (alias: string) => void;
    onCopyCommand: (cmd: string) => void;
  }

  let {
    alias,
    status,
    output,
    nowTick,
    onStart,
    onStop,
    onShowCredentials,
    onCopyCommand
  }: Props = $props();

  let outputBox: HTMLDivElement | null = $state(null);
  let userScrolled = $state(false);

  function onOutputScroll() {
    if (!outputBox) return;
    const atBottom =
      outputBox.scrollHeight - outputBox.scrollTop - outputBox.clientHeight < 8;
    userScrolled = !atBottom;
  }

  $effect(() => {
    void output;
    if (userScrolled) return;
    queueMicrotask(() => {
      if (outputBox) outputBox.scrollTop = outputBox.scrollHeight;
    });
  });

  function expiryHint(s: SessionStatus | undefined): string | null {
    if (!s || s.tokenRemainingSecs == null) return null;
    if (s.tokenRemainingSecs === 0) return 'expired';
    return formatDuration(s.tokenRemainingSecs);
  }

  let active = $derived(isActive(status));
  let port = $derived(alias ? portHint(alias) : null);
  let uptime = $derived.by(() => {
    void nowTick;
    return uptimeFrom(status?.startedAt ?? null);
  });
  let exp = $derived(expiryHint(status));
</script>

{#if !alias}
  <div class="flex h-full items-center justify-center text-sm text-muted-foreground">
    Select an alias to view details
  </div>
{:else}
  <div class="flex h-full flex-col">
    <header class="border-b border-border px-5 py-4">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <StatusDot tone={stateTone(status?.state)} pulse={status?.state === 'starting'} />
            <h1 class="truncate font-mono text-lg font-semibold">{alias.name}</h1>
            <Badge variant={kindBadgeVariant(alias.kind)}>{kindLabel(alias.kind)}</Badge>
            <Badge variant={stateTone(status?.state)}>{stateLabel(status?.state)}</Badge>
          </div>
          {#if alias.group || alias.subgroup}
            <p class="mt-1 text-xs text-muted-foreground">
              {alias.group ?? '—'}{alias.subgroup ? ' · ' + alias.subgroup : ''}
            </p>
          {/if}
        </div>
        <div class="flex shrink-0 items-center gap-1.5">
          {#if status?.hasCredentials}
            <Button variant="outline" size="sm" onclick={() => onShowCredentials(alias.name)}>
              <KeyRound class="h-3.5 w-3.5" /> Credentials
            </Button>
          {/if}
          {#if active}
            <Button variant="destructive" size="sm" onclick={() => onStop(alias)}>
              <Square class="h-3.5 w-3.5" /> Stop
            </Button>
          {:else}
            <Button size="sm" onclick={() => onStart(alias)}>
              <Play class="h-3.5 w-3.5" />
              {status?.state === 'expired' ? 'Re-login' : 'Start'}
            </Button>
          {/if}
        </div>
      </div>

      {#if status?.errorMessage}
        <div class="mt-3 rounded-md border border-status-error/30 bg-status-error/10 px-3 py-2 text-xs text-status-error">
          {status.errorMessage}
        </div>
      {/if}
    </header>

    <div class="grid grid-cols-2 gap-x-6 gap-y-2 px-5 py-4 text-xs sm:grid-cols-3">
      {#if active}
        <div>
          <div class="text-muted-foreground">Uptime</div>
          <div class="font-mono">{uptime}</div>
        </div>
      {/if}
      {#if status?.pid}
        <div>
          <div class="text-muted-foreground">PID</div>
          <div class="font-mono">{status.pid}</div>
        </div>
      {/if}
      {#if alias.profile}
        <div>
          <div class="text-muted-foreground">Profile</div>
          <div class="font-mono">{alias.profile}</div>
        </div>
      {/if}
      {#if alias.region}
        <div>
          <div class="text-muted-foreground">Region</div>
          <div class="font-mono">{alias.region}</div>
        </div>
      {/if}
      {#if alias.ssoSessionName}
        <div>
          <div class="text-muted-foreground">SSO Session</div>
          <div class="font-mono">{alias.ssoSessionName}</div>
        </div>
      {/if}
      {#if status?.identityArn}
        <div class="col-span-full">
          <div class="text-muted-foreground">Identity</div>
          <div class="break-all font-mono text-[11px]">{status.identityArn}</div>
        </div>
      {/if}
      {#if exp}
        <div>
          <div class="text-muted-foreground">Token</div>
          <div class={'font-mono ' + (status?.tokenRemainingSecs === 0 ? 'text-status-warn' : '')}>
            {exp === 'expired' ? 'expired' : 'expires in ' + exp}
          </div>
        </div>
      {/if}
      {#if alias.target}
        <div>
          <div class="text-muted-foreground">Target</div>
          <div class="break-all font-mono text-[11px]">{alias.target}</div>
        </div>
      {/if}
      {#if port}
        <div class="col-span-full">
          <div class="text-muted-foreground">Forwarding</div>
          <div class="font-mono text-status-info">{port}</div>
        </div>
      {/if}
    </div>

    <div class="border-y border-border bg-muted/30 px-5 py-3">
      <div class="flex items-center justify-between gap-2">
        <div class="text-[10px] uppercase tracking-wider text-muted-foreground">Command</div>
        <button
          type="button"
          onclick={() => onCopyCommand(alias.command)}
          class="inline-flex h-6 items-center gap-1 rounded-md px-1.5 text-[10px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          title="Copy command"
        >
          <Copy class="h-3 w-3" /> Copy
        </button>
      </div>
      <pre class="mt-1 overflow-x-auto whitespace-pre-wrap break-all font-mono text-[11px] text-muted-foreground">{alias.command}</pre>
    </div>

    <div class="flex min-h-0 flex-1 flex-col">
      <div class="flex items-center justify-between gap-2 px-5 py-2 text-[10px] uppercase tracking-wider text-muted-foreground">
        <div class="flex items-center gap-2">
          <TermIcon class="h-3 w-3" />
          <span>Output</span>
          <span class="normal-case text-muted-foreground/60">({output.length} lines)</span>
        </div>
        {#if userScrolled}
          <button
            type="button"
            class="text-[10px] normal-case text-primary hover:underline"
            onclick={() => {
              userScrolled = false;
              if (outputBox) outputBox.scrollTop = outputBox.scrollHeight;
            }}
          >
            Jump to bottom
          </button>
        {/if}
      </div>
      <div
        bind:this={outputBox}
        onscroll={onOutputScroll}
        class="min-h-0 flex-1 overflow-auto bg-[#0f1114] px-5 py-3 font-mono text-[11px] leading-relaxed"
      >
        {#if output.length === 0}
          <span class="italic text-muted-foreground">
            {active ? '(waiting for output…)' : '(no output — start the session to see output here)'}
          </span>
        {:else}
          {#each output as line, i (i)}
            <div class={'whitespace-pre-wrap ' + outputLineClass(line)}>{line}</div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}
