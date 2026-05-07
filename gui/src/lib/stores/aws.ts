import { writable, type Writable } from 'svelte/store';
import type { Alias, AwsProfile, Instance, Cluster, SessionStatus } from '$lib/types';

export const profile: Writable<string> = writable('default');
export const region: Writable<string> = writable('us-east-1');

export const aliases: Writable<Alias[]> = writable([]);
export const aliasesPath: Writable<string | null> = writable(null);
export const sessions: Writable<Record<string, SessionStatus>> = writable({});
export const instances: Writable<Instance[]> = writable([]);
export const clusters: Writable<Cluster[]> = writable([]);

/** Profiles parsed from ~/.aws/config and ~/.aws/credentials. */
export const awsProfiles: Writable<AwsProfile[]> = writable([]);
export const awsConfigPath: Writable<string | null> = writable(null);

export const loading = writable({
  aliases: false,
  instances: false,
  clusters: false
});
