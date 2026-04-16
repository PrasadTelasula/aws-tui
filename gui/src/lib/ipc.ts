/**
 * Thin, typed wrapper around Tauri's IPC surface.
 *
 * All calls into the Rust backend flow through this module. Components MUST
 * NOT import `@tauri-apps/api` directly — that way the set of available
 * commands stays discoverable and typed in one place.
 */
import { invoke } from '@tauri-apps/api/core';
import type {
  Alias,
  AliasesResponse,
  AppConfig,
  Cluster,
  Container,
  Instance,
  Service,
  SessionStatus,
  Task
} from './types';

export const ipc = {
  listAliases: (path?: string) =>
    invoke<AliasesResponse>('list_aliases', { path }),
  setAliasesPath: (path: string) =>
    invoke<AliasesResponse>('set_aliases_path', { path }),
  getConfig: () => invoke<AppConfig>('get_config'),
  startSession: (alias: string) => invoke<SessionStatus>('start_session', { alias }),
  stopSession: (alias: string) => invoke<SessionStatus>('stop_session', { alias }),
  listSessions: () => invoke<SessionStatus[]>('list_sessions'),

  listInstances: (profile?: string, region?: string) =>
    invoke<Instance[]>('list_instances', { profile, region }),
  describeInstance: (id: string) => invoke<unknown>('describe_instance', { id }),

  listClusters: (profile?: string, region?: string) =>
    invoke<Cluster[]>('list_clusters', { profile, region }),
  listServices: (cluster: string) => invoke<Service[]>('list_services', { cluster }),
  listTasks: (cluster: string, service?: string) =>
    invoke<Task[]>('list_tasks', { cluster, service }),
  listContainers: (taskArn: string) =>
    invoke<Container[]>('list_containers', { taskArn }),

  completeAwsCli: (line: string, cursor: number) =>
    invoke<string[]>('complete_aws_cli', { line, cursor }),
  awsWhoami: (profile?: string) => invoke<unknown>('aws_whoami', { profile })
};

export type Ipc = typeof ipc;
