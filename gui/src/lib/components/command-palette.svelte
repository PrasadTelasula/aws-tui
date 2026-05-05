<script lang="ts">
  import { tick } from 'svelte';
  import { goto } from '$app/navigation';
  import { aliases, instances } from '$lib/stores/aws';
  import { aliasMeta } from '$lib/sessions-helpers';
  import {
    MagnifyingGlass,
    Pulse,
    HardDrives,
    Stack,
    TerminalWindow
  } from 'phosphor-svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }
  let { open, onClose }: Props = $props();

  type ItemKind = 'nav' | 'alias' | 'inst';
  interface Item {
    kind: ItemKind;
    id: string;
    label: string;
    sub?: string;
    Icon: any;
    href: string;
  }

  const NAV_ITEMS: Item[] = [
    { kind: 'nav', id: 'nav-/',           label: 'Go to Sessions',    Icon: Pulse,          href: '/' },
    { kind: 'nav', id: 'nav-/instances',  label: 'Go to Instances',   Icon: HardDrives,     href: '/instances' },
    { kind: 'nav', id: 'nav-/containers', label: 'Go to Containers',  Icon: Stack,          href: '/containers' },
    { kind: 'nav', id: 'nav-/terminal',   label: 'Go to Terminal',    Icon: TerminalWindow, href: '/terminal' }
  ];

  let q = $state('');
  let cursor = $state(0);
  let inputEl: HTMLInputElement | null = $state(null);
  let resultsEl: HTMLDivElement | null = $state(null);

  let allItems = $derived.by<Item[]>(() => {
    const out: Item[] = [...NAV_ITEMS];
    for (const a of $aliases) {
      const meta = aliasMeta(a);
      out.push({
        kind: 'alias',
        id: `alias-${a.name}`,
        label: a.name,
        sub: a.command,
        Icon: meta.Icon,
        href: '/'
      });
    }
    for (const i of $instances) {
      out.push({
        kind: 'inst',
        id: `inst-${i.id}`,
        label: i.name ?? i.id,
        sub: i.id,
        Icon: HardDrives,
        href: '/instances'
      });
    }
    return out;
  });

  let filtered = $derived.by(() => {
    const f = q.trim().toLowerCase();
    if (!f) return allItems;
    return allItems.filter(
      (it) =>
        it.label.toLowerCase().includes(f) ||
        (it.sub?.toLowerCase().includes(f) ?? false)
    );
  });

  // Reset cursor when query changes
  $effect(() => {
    void filtered;
    cursor = 0;
  });

  // Focus input on open; reset state on close
  $effect(() => {
    if (open) {
      tick().then(() => inputEl?.focus());
    } else {
      q = '';
      cursor = 0;
    }
  });

  // Keep highlighted row in view
  $effect(() => {
    void cursor;
    void filtered;
    if (!resultsEl) return;
    queueMicrotask(() => {
      const el = resultsEl?.querySelector<HTMLElement>(`[data-cursor="${cursor}"]`);
      el?.scrollIntoView({ block: 'nearest' });
    });
  });

  function select(item: Item) {
    onClose();
    goto(item.href);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      cursor = Math.min(cursor + 1, filtered.length - 1);
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      cursor = Math.max(cursor - 1, 0);
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      const it = filtered[cursor];
      if (it) select(it);
    }
  }
</script>

{#if open}
  <div
    class="tui-cmd-overlay"
    onclick={onClose}
    role="presentation"
  >
    <div
      class="tui-cmd-panel"
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <div class="tui-cmd-input-row">
        <MagnifyingGlass size={14} weight="bold" />
        <input
          bind:this={inputEl}
          bind:value={q}
          onkeydown={onKeydown}
          class="tui-cmd-input"
          placeholder="Type to search anything…"
          spellcheck={false}
          autocomplete="off"
        />
        <kbd class="tui-kbd">esc</kbd>
      </div>

      <div bind:this={resultsEl} class="tui-cmd-results">
        {#each filtered.slice(0, 14) as it, i (it.id)}
          {@const ItemIcon = it.Icon}
          <button
            type="button"
            class="tui-cmd-result"
            class:is-active={i === cursor}
            data-cursor={i}
            onclick={() => select(it)}
            onmouseenter={() => (cursor = i)}
          >
            <ItemIcon size={13} weight="regular" />
            <span class="tui-cmd-result-label">{it.label}</span>
            {#if it.sub}
              <span class="tui-cmd-result-sub">{it.sub}</span>
            {/if}
            <span class="tui-cmd-result-kind">{it.kind}</span>
          </button>
        {/each}
        {#if filtered.length === 0}
          <div class="tui-cmd-empty">No matches</div>
        {/if}
      </div>

      <div class="tui-cmd-foot">
        <span><kbd class="tui-kbd">↑↓</kbd> navigate</span>
        <span><kbd class="tui-kbd">↵</kbd> select</span>
        <span><kbd class="tui-kbd">esc</kbd> dismiss</span>
      </div>
    </div>
  </div>
{/if}
