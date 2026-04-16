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
    case 'sso-login': return 'SSO';
    case 'ssm-session': return 'SSM';
    case 'iam-profile': return 'IAM';
    default: return 'OTHER';
  }
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

export interface AliasGroup {
  name: string;
  subgroups: { name: string; aliases: Alias[] }[];
}

export function groupAliases(aliases: Alias[]): AliasGroup[] {
  const map = new Map<string, Map<string, Alias[]>>();
  for (const a of aliases) {
    const g = a.group ?? 'Other';
    const sg = a.subgroup ?? kindLabel(a.kind);
    if (!map.has(g)) map.set(g, new Map());
    const sub = map.get(g)!;
    if (!sub.has(sg)) sub.set(sg, []);
    sub.get(sg)!.push(a);
  }
  return Array.from(map.entries()).map(([name, subs]) => ({
    name,
    subgroups: Array.from(subs.entries()).map(([sname, items]) => ({
      name: sname,
      aliases: items
    }))
  }));
}
