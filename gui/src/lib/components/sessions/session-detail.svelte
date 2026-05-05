<script lang="ts">
  import {
    Copy,
    Key as KeyRound,
    Play,
    Stop as Square,
    TerminalWindow as TermIcon,
    WarningCircle as AlertCircle
  } from 'phosphor-svelte';
  import StatusDot from '$lib/components/status-dot.svelte';
  import {
    aliasMeta,
    isActive,
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
  let pillTone = $derived(stateTone(status?.state));

  function classifyOutputLine(line: string): string {
    return outputLineClass(line);
  }
</script>

{#if !alias}
  <div class="tui-empty">
    <div class="tui-empty-icon">
      <TermIcon size={22} weight="thin" />
    </div>
    <div class="tui-empty-title">Select an alias</div>
    <div class="tui-empty-sub">Choose a session from the list to view details, output, and connection info.</div>
  </div>
{:else}
  {@const km = aliasMeta(alias)}
  {@const KindIcon = km.Icon}
  <div class="tui-detail-header">
    <div class="tui-detail-eyebrow">
      <span class={`tui-kind tui-kind-${km.tone}`}>
        <KindIcon size={10} weight="bold" />
        {km.label}
      </span>
      {#if alias.group || alias.subgroup}
        <span>·</span>
        <span class="tui-detail-eyebrow-path">
          {alias.group ?? '—'}{alias.subgroup ? ' / ' + alias.subgroup : ''}
        </span>
      {/if}
    </div>
    <div class="tui-detail-title-row">
      <h1 class="tui-detail-title">
        <span>{alias.name}</span>
        <span class={`tui-pill tui-pill-${pillTone} tui-pill-md`}>
          <StatusDot tone={pillTone} size={6} />
          {stateLabel(status?.state)}
        </span>
      </h1>
      <div class="tui-detail-actions">
        {#if status?.hasCredentials}
          <button
            type="button"
            class="tui-btn tui-btn-outline tui-btn-sm"
            onclick={() => onShowCredentials(alias.name)}
          >
            <KeyRound size={12} weight="regular" />
            Credentials
          </button>
        {/if}
        <button
          type="button"
          class="tui-btn tui-btn-ghost tui-btn-sm"
          title="Copy command"
          onclick={() => onCopyCommand(alias.command)}
        >
          <Copy size={12} weight="regular" />
        </button>
        {#if active}
          <button
            type="button"
            class="tui-btn tui-btn-destructive tui-btn-sm"
            onclick={() => onStop(alias)}
          >
            <Square size={12} weight="regular" />
            Stop
          </button>
        {:else}
          <button
            type="button"
            class="tui-btn tui-btn-default tui-btn-sm"
            onclick={() => onStart(alias)}
          >
            <Play size={12} weight="regular" />
            {status?.state === 'expired' ? 'Re-login' : 'Start'}
          </button>
        {/if}
      </div>
    </div>
    {#if status?.errorMessage}
      <div class="tui-detail-error">
        <span style="display: inline-flex; flex-shrink: 0; margin-top: 1px;">
          <AlertCircle size={14} weight="regular" />
        </span>
        <span>{status.errorMessage}</span>
      </div>
    {/if}
  </div>

  <div class="tui-meta-grid">
    {#if active}
      <div class="tui-meta-cell">
        <span class="tui-meta-label">Uptime</span>
        <span class="tui-meta-value">{uptime}</span>
      </div>
    {/if}
    {#if status?.pid}
      <div class="tui-meta-cell">
        <span class="tui-meta-label">PID</span>
        <span class="tui-meta-value">{status.pid}</span>
      </div>
    {/if}
    {#if alias.profile}
      <div class="tui-meta-cell">
        <span class="tui-meta-label">Profile</span>
        <span class="tui-meta-value">{alias.profile}</span>
      </div>
    {/if}
    {#if alias.region}
      <div class="tui-meta-cell">
        <span class="tui-meta-label">Region</span>
        <span class="tui-meta-value">{alias.region}</span>
      </div>
    {/if}
    {#if alias.ssoSessionName}
      <div class="tui-meta-cell">
        <span class="tui-meta-label">SSO Session</span>
        <span class="tui-meta-value">{alias.ssoSessionName}</span>
      </div>
    {/if}
    {#if exp}
      <div class="tui-meta-cell">
        <span class="tui-meta-label">Token</span>
        <span class={`tui-meta-value ${status?.tokenRemainingSecs === 0 ? 'is-warn' : ''}`}>
          {exp === 'expired' ? 'expired' : 'expires in ' + exp}
        </span>
      </div>
    {/if}
    {#if alias.target}
      <div class="tui-meta-cell">
        <span class="tui-meta-label">Target</span>
        <span class="tui-meta-value">{alias.target}</span>
      </div>
    {/if}
    {#if port}
      <div class="tui-meta-cell">
        <span class="tui-meta-label">Forwarding</span>
        <span class="tui-meta-value is-info">{port}</span>
      </div>
    {/if}
    {#if status?.identityArn}
      <div class="tui-meta-cell is-wide">
        <span class="tui-meta-label">Identity</span>
        <span class="tui-meta-value is-wrap">{status.identityArn}</span>
      </div>
    {/if}
  </div>

  <div class="tui-command-block">
    <div class="tui-command-block-head">
      <span class="tui-command-block-label">
        <TermIcon size={11} weight="bold" />
        Command
      </span>
      <button
        type="button"
        class="tui-btn tui-btn-ghost tui-btn-sm"
        onclick={() => onCopyCommand(alias.command)}
        title="Copy command"
      >
        <Copy size={11} weight="regular" />
        Copy
      </button>
    </div>
    <pre class="tui-command-block-body"><span class="tui-command-block-prompt">$ </span>{alias.command}</pre>
  </div>

  <div class="tui-output-wrap">
    <div class="tui-output-head">
      <div class="tui-output-head-tabs">
        <strong>Output</strong>
      </div>
      <div class="tui-output-head-meta">
        <span>{output.length} lines</span>
        {#if active}
          <span style="display: inline-flex; align-items: center; gap: 5px; color: var(--tui-ok);">
            <StatusDot tone="ok" pulse size={5} />
            streaming
          </span>
        {:else if userScrolled}
          <button
            type="button"
            class="tui-btn tui-btn-ghost tui-btn-sm"
            onclick={() => {
              userScrolled = false;
              if (outputBox) outputBox.scrollTop = outputBox.scrollHeight;
            }}
          >
            Jump to bottom
          </button>
        {/if}
      </div>
    </div>
    <div bind:this={outputBox} onscroll={onOutputScroll} class="tui-output-body">
      {#if output.length === 0}
        <span class="tui-output-empty">
          {active ? '(waiting for output…)' : '(no output — start the session to see output here)'}
        </span>
      {:else}
        {#each output as line, i (i)}
          {@const isRecent = active && i === output.length - 1}
          <div class={`tui-output-line ${classifyOutputLine(line)}`} class:is-recent={isRecent}>
            {line}
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}
