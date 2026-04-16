import type { Component } from 'svelte';
import {
  Boxes,
  Database,
  KeyRound,
  Network,
  Server,
  Shield,
  TerminalSquare,
  Tag
} from 'lucide-svelte';
import type { Alias, SessionState, SessionStatus } from './types';

export type StatusTone = 'ok' | 'warn' | 'error' | 'info' | 'muted';

export function stateTone(s: SessionState | undefined): StatusTone {
  switch (s) {
    case 'connected': return 'ok';
    case 'running': return 'info';
    case 'starting': return 'info';
    case 'expired': return 'warn';
    case 'error': return 'error';
    default: return 'muted';
  }
}

export function stateLabel(s: SessionState | undefined): string {
  return s ?? 'stopped';
}

export function isActive(status: SessionStatus | undefined): boolean {
  if (!status) return false;
  return ['running', 'starting', 'connected'].includes(status.state);
}

export function kindBadgeVariant(k: Alias['kind']): 'info' | 'ok' | 'warn' | 'muted' {
  switch (k) {
    case 'sso-login': return 'info';
    case 'ssm-session': return 'ok';
    case 'iam-profile': return 'warn';
    default: return 'muted';
  }
}

export function kindLabel(k: Alias['kind']): string {
  switch (k) {
    case 'sso-login': return 'SSO Login';
    case 'ssm-session': return 'SSM Session';
    case 'iam-profile': return 'IAM Profile';
    default: return 'Other';
  }
}

export function kindIcon(k: Alias['kind']): Component {
  switch (k) {
    case 'sso-login': return Shield;
    case 'ssm-session': return Network;
    case 'iam-profile': return KeyRound;
    default: return Tag;
  }
}

export function subgroupIcon(name: string): Component {
  const s = name.toLowerCase();
  if (/sso|login|auth/.test(s)) return Shield;
  if (/db|database|rds|postgres|mysql/.test(s)) return Database;
  if (/os|opensearch|elastic/.test(s)) return Boxes;
  if (/shell|term|host|ec2|instance/.test(s)) return TerminalSquare;
  if (/vpn|tunnel|net/.test(s)) return Network;
  if (/iam|key|cred/.test(s)) return KeyRound;
  if (/ecs|container|task|cluster|service/.test(s)) return Server;
  return Tag;
}

export function portHint(a: Alias): string | null {
  if (a.kind !== 'ssm-session') return null;
  if (a.ssmLocalPort && a.ssmHost && a.ssmRemotePort) {
    return `:${a.ssmLocalPort} → ${a.ssmHost}:${a.ssmRemotePort}`;
  }
  if (a.ssmLocalPort && a.ssmRemotePort) {
    return `:${a.ssmLocalPort} → :${a.ssmRemotePort}`;
  }
  if (a.target) return a.target;
  return null;
}

export function outputLineClass(line: string): string {
  if (line.startsWith('>>>')) return 'text-status-info';
  if (line.startsWith('[stderr]')) return 'text-status-warn';
  const lower = line.toLowerCase();
  if (
    lower.includes('error') ||
    lower.includes('failed') ||
    lower.includes('expired') ||
    lower.includes('denied')
  ) {
    return 'text-status-error';
  }
  return 'text-[#d4d4d4]';
}

// ─── Grouping ─────────────────────────────────────────────────────────

export interface AliasSubgroup {
  name: string;
  aliases: Alias[];
}

export interface AliasGroup {
  name: string;
  /** Lucide icon for this group header. */
  icon: Component;
  /** True when this group came from a `# group:` directive (vs. inferred). */
  explicit: boolean;
  subgroups: AliasSubgroup[];
}

/**
 * Group aliases for display.
 *
 * Rules:
 *   1. Aliases with an explicit `# group: X type: Y` directive are grouped
 *      by that group, then by `type` (subgroup) within it.
 *   2. Aliases without an explicit group are grouped by KIND (SSO Logins /
 *      SSM Sessions / IAM Profiles / Other) and sub-grouped by an inferred
 *      bucket from the alias name (e.g. shared name prefix). This means a
 *      flat unannotated file still produces meaningful sections.
 */
export function groupAliases(aliases: Alias[]): AliasGroup[] {
  const explicit = new Map<string, Map<string, Alias[]>>();
  const inferred = new Map<Alias['kind'], Map<string, Alias[]>>();

  for (const a of aliases) {
    if (a.group) {
      const sg = a.subgroup ?? '—';
      if (!explicit.has(a.group)) explicit.set(a.group, new Map());
      const m = explicit.get(a.group)!;
      if (!m.has(sg)) m.set(sg, []);
      m.get(sg)!.push(a);
    } else {
      const bucket = inferBucket(a);
      if (!inferred.has(a.kind)) inferred.set(a.kind, new Map());
      const m = inferred.get(a.kind)!;
      if (!m.has(bucket)) m.set(bucket, []);
      m.get(bucket)!.push(a);
    }
  }

  const out: AliasGroup[] = [];

  // Explicit groups first, alphabetical
  const explicitNames = Array.from(explicit.keys()).sort();
  for (const name of explicitNames) {
    const subs = explicit.get(name)!;
    const subgroups: AliasSubgroup[] = Array.from(subs.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([sname, items]) => ({ name: sname, aliases: items }));
    out.push({ name, icon: subgroupIcon(name), explicit: true, subgroups });
  }

  // Inferred groups, in a deterministic kind order
  const kindOrder: Alias['kind'][] = ['sso-login', 'ssm-session', 'iam-profile', 'other'];
  for (const k of kindOrder) {
    const subs = inferred.get(k);
    if (!subs) continue;
    const subgroups: AliasSubgroup[] = Array.from(subs.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([sname, items]) => ({ name: sname, aliases: items }));
    out.push({
      name: kindLabel(k),
      icon: kindIcon(k),
      explicit: false,
      subgroups
    });
  }

  return out;
}

/**
 * Heuristic bucket name for an alias when no explicit subgroup is set.
 * Tries (in order): SSO session prefix → host short-name → alias name prefix.
 */
function inferBucket(a: Alias): string {
  if (a.kind === 'sso-login' && a.ssoSessionName) {
    // sso-dmsc-prd → "dmsc"
    const parts = a.ssoSessionName.split('-').filter(Boolean);
    if (parts[0]?.toLowerCase() === 'sso' && parts[1]) return parts[1];
    if (parts[0]) return parts[0];
  }
  if (a.kind === 'ssm-session') {
    if (a.ssmHost) {
      // db.int.registry-svc.example.com → "registry-svc"
      const segs = a.ssmHost.split('.').filter(Boolean);
      if (segs.length >= 2) return segs[segs.length - 3] ?? segs[0];
      return segs[0] ?? 'ssm';
    }
    // dmscdbint → "dmscdb"
    const m = a.name.match(/^([a-z]+?(?:db|os|opensearch|shell|host))/i);
    if (m) return m[1].toLowerCase();
  }
  // Generic: shared alphabetic prefix
  const m = a.name.match(/^([a-z]+)/i);
  if (m) return m[1].toLowerCase();
  return 'misc';
}
