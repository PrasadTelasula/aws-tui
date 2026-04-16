import { writable, type Writable } from 'svelte/store';
import type { Alias, Instance, Cluster, SessionStatus } from '$lib/types';

export const profile: Writable<string> = writable('default');
export const region: Writable<string> = writable('us-east-1');

export const aliases: Writable<Alias[]> = writable([]);
export const sessions: Writable<Record<string, SessionStatus>> = writable({});
export const instances: Writable<Instance[]> = writable([]);
export const clusters: Writable<Cluster[]> = writable([]);

export const loading = writable({
  aliases: false,
  instances: false,
  clusters: false
});
