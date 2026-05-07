<script lang="ts">
  import { onMount } from 'svelte';
  import { ipc } from '$lib/ipc';
  import { aliases as aliasesStore, aliasesPath } from '$lib/stores/aws';
  import { aliasMeta } from '$lib/sessions-helpers';
  import type { Alias, AliasKind } from '$lib/types';
  import {
    GearSix,
    FolderOpen,
    Plus,
    PencilSimple,
    Trash,
    FloppyDisk,
    ArrowCounterClockwise,
    X,
    Check,
    WarningCircle
  } from 'phosphor-svelte';

  /** Editable copy of the loaded aliases. */
  let working = $state<Alias[]>([]);
  let originalJson = $state('[]');
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let savedAt = $state<number | null>(null);

  /** index in `working` being edited; -1 = new, null = no editor open */
  let editing = $state<number | null>(null);

  /** Form state, generic over all kinds. */
  type Form = {
    name: string;
    kind: AliasKind;
    group: string;
    subgroup: string;
    profile: string;
    region: string;
    target: string;
    ssoSessionName: string;
    ssmHost: string;
    ssmRemotePort: string;
    ssmLocalPort: string;
    ssmDocument: string;
    /** Used for kind='other' — full raw command. */
    command: string;
  };
  const blankForm = (): Form => ({
    name: '',
    kind: 'ssm-session',
    group: '',
    subgroup: '',
    profile: '',
    region: '',
    target: '',
    ssoSessionName: '',
    ssmHost: '',
    ssmRemotePort: '',
    ssmLocalPort: '',
    ssmDocument: '',
    command: ''
  });
  let form = $state<Form>(blankForm());
  let formError = $state<string | null>(null);

  onMount(() => {
    working = JSON.parse(JSON.stringify($aliasesStore));
    originalJson = JSON.stringify(working);
  });

  let dirty = $derived(JSON.stringify(working) !== originalJson);

  function aliasToForm(a: Alias): Form {
    return {
      name: a.name,
      kind: a.kind,
      group: a.group ?? '',
      subgroup: a.subgroup ?? '',
      profile: a.profile ?? '',
      region: a.region ?? '',
      target: a.target ?? '',
      ssoSessionName: a.ssoSessionName ?? '',
      ssmHost: a.ssmHost ?? '',
      ssmRemotePort: a.ssmRemotePort ?? '',
      ssmLocalPort: a.ssmLocalPort ?? '',
      ssmDocument: a.ssmDocument ?? '',
      command: a.command
    };
  }

  function buildCommand(f: Form): string {
    if (f.kind === 'other') return f.command.trim();
    if (f.kind === 'sso-login') {
      const session = f.ssoSessionName.trim();
      return session
        ? `aws sso login --sso-session ${session}`
        : 'aws sso login';
    }
    if (f.kind === 'iam-profile') {
      const p = f.profile.trim() || f.name.trim();
      return `aws sts get-caller-identity --profile ${p}`;
    }
    // ssm-session
    let cmd = `aws ssm start-session --target ${f.target.trim()}`;
    const doc = f.ssmDocument.trim();
    if (doc) cmd += ` --document-name ${doc}`;
    const host = f.ssmHost.trim();
    const remote = f.ssmRemotePort.trim();
    const local = f.ssmLocalPort.trim() || remote;
    if (host && remote) {
      cmd += ` --parameters '{"host":["${host}"],"portNumber":["${remote}"],"localPortNumber":["${local}"]}'`;
    } else if (remote) {
      cmd += ` --parameters '{"portNumber":["${remote}"],"localPortNumber":["${local}"]}'`;
    }
    if (f.profile.trim()) cmd += ` --profile ${f.profile.trim()}`;
    if (f.region.trim()) cmd += ` --region ${f.region.trim()}`;
    return cmd;
  }

  function formToAlias(f: Form): Alias {
    return {
      name: f.name.trim(),
      command: buildCommand(f),
      kind: f.kind,
      group: f.group.trim() || null,
      subgroup: f.subgroup.trim() || null,
      profile: f.profile.trim() || (f.kind === 'iam-profile' ? f.name.trim() : null),
      region: f.region.trim() || null,
      target: f.target.trim() || null,
      ssoSessionName: f.ssoSessionName.trim() || null,
      ssmDocument: f.ssmDocument.trim() || null,
      ssmLocalPort: f.ssmLocalPort.trim() || null,
      ssmRemotePort: f.ssmRemotePort.trim() || null,
      ssmHost: f.ssmHost.trim() || null
    };
  }

  function startEdit(i: number) {
    editing = i;
    form = aliasToForm(working[i]);
    formError = null;
  }
  function startAdd() {
    editing = -1;
    form = blankForm();
    formError = null;
  }
  function cancelEdit() {
    editing = null;
    formError = null;
  }
  function commitForm() {
    formError = null;
    const trimmedName = form.name.trim();
    if (!trimmedName) { formError = 'Name is required'; return; }
    if (!/^[a-zA-Z0-9_-]+$/.test(trimmedName)) {
      formError = 'Name can only contain letters, digits, underscore, dash';
      return;
    }
    // duplicate check (allow keeping the same name when editing)
    const dupIdx = working.findIndex((a, i) => a.name === trimmedName && i !== editing);
    if (dupIdx !== -1) { formError = `An alias named "${trimmedName}" already exists`; return; }
    if (form.kind === 'ssm-session' && !form.target.trim()) {
      formError = 'SSM aliases need a target instance ID';
      return;
    }
    if (form.kind === 'sso-login' && !form.ssoSessionName.trim()) {
      formError = 'SSO logins need an SSO session name';
      return;
    }
    if (form.kind === 'other' && !form.command.trim()) {
      formError = 'Command is required for "other" aliases';
      return;
    }

    const next = formToAlias(form);
    if (editing === -1) {
      working = [...working, next];
    } else if (editing != null) {
      working = working.map((a, i) => (i === editing ? next : a));
    }
    editing = null;
  }
  function deleteAt(i: number) {
    working = working.filter((_, idx) => idx !== i);
  }
  function revert() {
    working = JSON.parse(originalJson);
  }

  async function save() {
    saving = true;
    saveError = null;
    try {
      const resp = await ipc.saveAliases($aliasesPath, working);
      aliasesStore.set(resp.aliases);
      aliasesPath.set(resp.path);
      working = JSON.parse(JSON.stringify(resp.aliases));
      originalJson = JSON.stringify(working);
      savedAt = Date.now();
      setTimeout(() => { savedAt = null; }, 2500);
    } catch (e) {
      saveError = String(e);
    } finally {
      saving = false;
    }
  }

  async function pickFile() {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({
      multiple: false,
      directory: false,
      title: 'Select aliases file',
      filters: [
        { name: 'Aliases', extensions: ['sh', 'bash', 'zsh', 'aliases', 'txt'] },
        { name: 'All files', extensions: ['*'] }
      ]
    });
    if (typeof selected !== 'string') return;
    const resp = await ipc.setAliasesPath(selected);
    aliasesStore.set(resp.aliases);
    aliasesPath.set(resp.path);
    working = JSON.parse(JSON.stringify(resp.aliases));
    originalJson = JSON.stringify(working);
  }

  function summary(a: Alias): string {
    if (a.kind === 'sso-login') return a.ssoSessionName ?? '—';
    if (a.kind === 'iam-profile') return a.profile ?? '—';
    if (a.kind === 'ssm-session') {
      const p = a.ssmHost && a.ssmRemotePort ? `${a.ssmHost}:${a.ssmRemotePort}` : a.target ?? '—';
      return p;
    }
    return a.command.split(/\s+/).slice(0, 4).join(' ') + (a.command.split(/\s+/).length > 4 ? '…' : '');
  }
</script>

<div class="tui-screen">
  <!-- Toolbar -->
  <div class="tui-toolbar">
    <div class="tui-toolbar-title">
      <span class="tui-toolbar-title-icon"><GearSix size={15} weight="regular" /></span>
      Settings
    </div>
    <div class="tui-toolbar-stats">
      <span class="tui-stat"><strong>{working.length}</strong> aliases</span>
      {#if dirty}
        <span class="tui-stat tui-stat-warn">unsaved changes</span>
      {:else if savedAt}
        <span class="tui-stat tui-stat-ok"><Check size={11} weight="bold" /> saved</span>
      {/if}
    </div>
    <div class="tui-toolbar-spacer"></div>
    {#if dirty}
      <button type="button" class="tui-btn tui-btn-ghost tui-btn-sm" onclick={revert} disabled={saving}>
        <ArrowCounterClockwise size={12} weight="regular" />
        Revert
      </button>
    {/if}
    <button
      type="button"
      class="tui-btn tui-btn-default tui-btn-sm"
      onclick={save}
      disabled={!dirty || saving}
      title="Write changes back to the aliases file"
    >
      <FloppyDisk size={12} weight="regular" />
      {saving ? 'Saving…' : 'Save changes'}
    </button>
  </div>

  {#if saveError}
    <div style="padding: 8px 14px; font-size: 12px; color: var(--tui-err); background: var(--tui-err-soft); border-bottom: 1px solid rgba(242,107,107,0.2); display: flex; gap: 8px; align-items: center;">
      <WarningCircle size={14} weight="bold" />
      {saveError}
    </div>
  {/if}

  <!-- Content split -->
  <div class="tui-settings-body">
    <!-- Aliases list / editor -->
    <section class="tui-settings-section">
      <header class="tui-settings-section-head">
        <div>
          <h2 class="tui-settings-section-title">Aliases</h2>
          <p class="tui-settings-section-sub">
            Stored at
            <button type="button" class="tui-settings-path" onclick={pickFile} title="Change file">
              <FolderOpen size={11} weight="regular" />
              <span>{$aliasesPath ?? '~/.aws_tui_config'}</span>
            </button>
          </p>
        </div>
        <button type="button" class="tui-btn tui-btn-outline tui-btn-sm" onclick={startAdd}>
          <Plus size={12} weight="bold" />
          Add alias
        </button>
      </header>

      {#if working.length === 0}
        <div class="tui-empty" style="padding: 32px;">
          <div class="tui-empty-title">No aliases yet</div>
          <div class="tui-empty-sub">Click <strong>Add alias</strong> to create your first one. Saving will write the file to disk.</div>
        </div>
      {:else}
        <div class="tui-settings-table">
          <div class="tui-settings-table-row tui-settings-table-head">
            <span style="width: 110px;">Kind</span>
            <span style="flex: 1;">Name</span>
            <span style="flex: 1;">Profile / Region</span>
            <span style="flex: 2;">Summary</span>
            <span style="width: 80px;"></span>
          </div>
          {#each working as a, i (a.name)}
            {@const meta = aliasMeta(a)}
            {@const Icon = meta.Icon}
            <div class="tui-settings-table-row">
              <span style="width: 110px;">
                <span class={`tui-kind tui-kind-${meta.tone}`}>
                  <Icon size={10} weight="bold" />
                  {meta.label}
                </span>
              </span>
              <span style="flex: 1; font-family: var(--tui-font-mono); font-size: 12px;">{a.name}</span>
              <span style="flex: 1; font-family: var(--tui-font-mono); font-size: 11px; color: var(--tui-fg-3);">
                {a.profile ?? '—'}{a.region ? ' · ' + a.region : ''}
              </span>
              <span style="flex: 2; font-family: var(--tui-font-mono); font-size: 11px; color: var(--tui-fg-4); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;" title={a.command}>
                {summary(a)}
              </span>
              <span style="width: 80px; display: flex; gap: 4px; justify-content: flex-end;">
                <button type="button" class="tui-iconbtn tui-iconbtn-sm" title="Edit" onclick={() => startEdit(i)}>
                  <PencilSimple size={12} weight="regular" />
                </button>
                <button type="button" class="tui-iconbtn tui-iconbtn-sm is-danger" title="Delete" onclick={() => deleteAt(i)}>
                  <Trash size={12} weight="regular" />
                </button>
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </div>

  <!-- Edit / Add form drawer -->
  {#if editing != null}
    <div class="tui-settings-drawer-overlay" role="presentation" onclick={cancelEdit}>
      <aside class="tui-settings-drawer" role="dialog" aria-modal="true" onclick={(e) => e.stopPropagation()}>
        <header class="tui-settings-drawer-head">
          <div>
            <div class="tui-settings-section-title">{editing === -1 ? 'New alias' : 'Edit alias'}</div>
            <div class="tui-settings-section-sub">{editing === -1 ? 'Pick a kind, fill in the fields, and save.' : 'Changes apply to your in-memory list — save to write the file.'}</div>
          </div>
          <button type="button" class="tui-iconbtn tui-iconbtn-md" onclick={cancelEdit} aria-label="Cancel">
            <X size={14} weight="bold" />
          </button>
        </header>

        <div class="tui-settings-form">
          <label class="tui-field">
            <span class="tui-field-label">Kind</span>
            <select class="tui-field-input" bind:value={form.kind}>
              <option value="sso-login">SSO login</option>
              <option value="iam-profile">IAM profile</option>
              <option value="ssm-session">SSM (port-forward / shell)</option>
              <option value="other">Other (raw command)</option>
            </select>
          </label>

          <label class="tui-field">
            <span class="tui-field-label">Alias name <em class="tui-field-req">*</em></span>
            <input class="tui-field-input" type="text" bind:value={form.name} placeholder="rds-prod" spellcheck={false} />
            <span class="tui-field-hint">Letters, digits, underscore, dash. Must be unique.</span>
          </label>

          <div class="tui-field-row">
            <label class="tui-field" style="flex: 1;">
              <span class="tui-field-label">Group</span>
              <input class="tui-field-input" type="text" bind:value={form.group} placeholder="SSM" spellcheck={false} />
            </label>
            <label class="tui-field" style="flex: 1;">
              <span class="tui-field-label">Subgroup</span>
              <input class="tui-field-input" type="text" bind:value={form.subgroup} placeholder="Databases" spellcheck={false} />
            </label>
          </div>

          {#if form.kind === 'sso-login'}
            <label class="tui-field">
              <span class="tui-field-label">SSO session <em class="tui-field-req">*</em></span>
              <input class="tui-field-input" type="text" bind:value={form.ssoSessionName} placeholder="company-prod" spellcheck={false} />
              <span class="tui-field-hint">From <code>~/.aws/config</code> — the <code>[sso-session NAME]</code> block.</span>
            </label>
          {/if}

          {#if form.kind === 'iam-profile'}
            <label class="tui-field">
              <span class="tui-field-label">Profile name</span>
              <input class="tui-field-input" type="text" bind:value={form.profile} placeholder="prod-admin" spellcheck={false} />
              <span class="tui-field-hint">Defaults to the alias name if blank.</span>
            </label>
          {/if}

          {#if form.kind === 'ssm-session'}
            <label class="tui-field">
              <span class="tui-field-label">Target instance <em class="tui-field-req">*</em></span>
              <input class="tui-field-input" type="text" bind:value={form.target} placeholder="i-0a9c8f7e6d5b4a321" spellcheck={false} />
            </label>
            <div class="tui-field-row">
              <label class="tui-field" style="flex: 1;">
                <span class="tui-field-label">Profile</span>
                <input class="tui-field-input" type="text" bind:value={form.profile} spellcheck={false} />
              </label>
              <label class="tui-field" style="flex: 1;">
                <span class="tui-field-label">Region</span>
                <input class="tui-field-input" type="text" bind:value={form.region} placeholder="us-east-1" spellcheck={false} />
              </label>
            </div>
            <label class="tui-field">
              <span class="tui-field-label">SSM document</span>
              <input class="tui-field-input" type="text" bind:value={form.ssmDocument} placeholder="AWS-StartPortForwardingSessionToRemoteHost" spellcheck={false} />
              <span class="tui-field-hint">Leave blank for an interactive shell session.</span>
            </label>
            <label class="tui-field">
              <span class="tui-field-label">Remote host</span>
              <input class="tui-field-input" type="text" bind:value={form.ssmHost} placeholder="prod-db.cluster-xyz.rds.amazonaws.com" spellcheck={false} />
            </label>
            <div class="tui-field-row">
              <label class="tui-field" style="flex: 1;">
                <span class="tui-field-label">Remote port</span>
                <input class="tui-field-input" type="text" bind:value={form.ssmRemotePort} placeholder="5432" spellcheck={false} />
              </label>
              <label class="tui-field" style="flex: 1;">
                <span class="tui-field-label">Local port</span>
                <input class="tui-field-input" type="text" bind:value={form.ssmLocalPort} placeholder="5432" spellcheck={false} />
                <span class="tui-field-hint">Defaults to the remote port.</span>
              </label>
            </div>
          {/if}

          {#if form.kind === 'other'}
            <label class="tui-field">
              <span class="tui-field-label">Command <em class="tui-field-req">*</em></span>
              <textarea class="tui-field-input" rows="4" bind:value={form.command} spellcheck={false}></textarea>
              <span class="tui-field-hint">Stored verbatim. Will not be re-parsed for ports/profiles/etc.</span>
            </label>
          {/if}

          <!-- Live preview of the generated command -->
          <div class="tui-field">
            <span class="tui-field-label">Generated command</span>
            <pre class="tui-field-preview">{buildCommand(form) || '—'}</pre>
          </div>

          {#if formError}
            <div class="tui-field-error">
              <WarningCircle size={12} weight="bold" />
              {formError}
            </div>
          {/if}
        </div>

        <footer class="tui-settings-drawer-foot">
          <button type="button" class="tui-btn tui-btn-ghost tui-btn-sm" onclick={cancelEdit}>Cancel</button>
          <button type="button" class="tui-btn tui-btn-default tui-btn-sm" onclick={commitForm}>
            <Check size={12} weight="bold" />
            {editing === -1 ? 'Add' : 'Update'}
          </button>
        </footer>
      </aside>
    </div>
  {/if}
</div>
