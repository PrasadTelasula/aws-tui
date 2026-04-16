import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

export function formatRelative(iso: string | null | undefined): string {
  if (!iso) return '—';
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return '—';
  const diff = (Date.now() - then) / 1000;
  const abs = Math.abs(diff);
  const sign = diff >= 0 ? 'ago' : 'from now';
  if (abs < 60) return `${Math.floor(abs)}s ${sign}`;
  if (abs < 3600) return `${Math.floor(abs / 60)}m ${sign}`;
  if (abs < 86400) return `${Math.floor(abs / 3600)}h ${sign}`;
  return `${Math.floor(abs / 86400)}d ${sign}`;
}

export function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n - 1) + '…' : s;
}

export function formatDuration(secs: number): string {
  if (secs < 0) return '0s';
  if (secs < 60) return `${Math.floor(secs)}s`;
  if (secs < 3600) {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return s ? `${m}m ${s}s` : `${m}m`;
  }
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return m ? `${h}h ${m}m` : `${h}h`;
}

export function uptimeFrom(iso: string | null): string {
  if (!iso) return '—';
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return '—';
  const secs = Math.max(0, (Date.now() - t) / 1000);
  return formatDuration(secs);
}
