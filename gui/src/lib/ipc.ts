/**
 * Thin, typed wrapper around Tauri's IPC surface.
 *
 * All calls into the Rust backend flow through this module. Components MUST
 * NOT import `@tauri-apps/api` directly — that way the set of available
 * commands stays discoverable and typed in one place.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Alias,
  AliasKind,
  AliasesResponse,
  AppConfig,
  Cluster,
  Container,
  CredentialInfo,
  Instance,
  Service,
  SessionStatus,
  Task
} from './types';

export interface PtyOpenOpts {
  shell?: string;
  cwd?: string;
  rows?: number;
  cols?: number;
  profile?: string;
  region?: string;
}

export const ipc = {
  // -------- aliases / config --------
  listAliases: (path?: string) => invoke<AliasesResponse>('list_aliases', { path }),
  setAliasesPath: (path: string) => invoke<AliasesResponse>('set_aliases_path', { path }),
  saveAliases: (path: string | null, aliases: Alias[]) =>
    invoke<AliasesResponse>('save_aliases', { path, aliases }),
  getConfig: () => invoke<AppConfig>('get_config'),

  // -------- sessions --------
  startSession: (
    alias: string,
    command: string,
    kind: AliasKind,
    ssoSessionName?: string | null,
    profileName?: string | null
  ) =>
    invoke<SessionStatus>('start_session', {
      alias,
      command,
      kind,
      ssoSessionName: ssoSessionName ?? null,
      profileName: profileName ?? null
    }),
  stopSession: (alias: string) => invoke<SessionStatus>('stop_session', { alias }),
  stopAllSessions: () => invoke<number>('stop_all_sessions'),
  listSessions: () => invoke<SessionStatus[]>('list_sessions'),
  sessionOutput: (alias: string) => invoke<string[]>('session_output', { alias }),
  getCredentials: (alias: string) =>
    invoke<CredentialInfo | null>('get_credentials', { alias }),
  checkExistingSso: (aliases: Array<[string, string]>) =>
    invoke<SessionStatus[]>('check_existing_sso', { aliases }),
  checkExistingIam: (aliases: Array<[string, string]>) =>
    invoke<SessionStatus[]>('check_existing_iam', { aliases }),
  onSessionOutput: (alias: string, cb: (line: string) => void) =>
    listen<string>(`session://${alias}/output`, (e) => cb(e.payload)),
  onSessionStatus: (alias: string, cb: (s: SessionStatus) => void) =>
    listen<SessionStatus>(`session://${alias}/status`, (e) => cb(e.payload)),
  onSessionsChanged: (cb: () => void) =>
    listen<void>('sessions://changed', () => cb()),

  // -------- AWS browsers --------
  listInstances: (profile?: string, region?: string) =>
    invoke<Instance[]>('list_instances', { profile, region }),
  describeInstance: (id: string, profile?: string, region?: string) =>
    invoke<unknown>('describe_instance', { id, profile, region }),
  listClusters: (profile?: string, region?: string) =>
    invoke<Cluster[]>('list_clusters', { profile, region }),
  listServices: (cluster: string, profile?: string, region?: string) =>
    invoke<Service[]>('list_services', { cluster, profile, region }),
  listTasks: (cluster: string, service?: string, profile?: string, region?: string) =>
    invoke<Task[]>('list_tasks', { cluster, service, profile, region }),
  listContainers: (taskArn: string, cluster: string, profile?: string, region?: string) =>
    invoke<Container[]>('list_containers', { taskArn, cluster, profile, region }),

  completeAwsCli: (line: string, cursor: number) =>
    invoke<string[]>('complete_aws_cli', { line, cursor }),
  awsWhoami: (profile?: string) => invoke<unknown>('aws_whoami', { profile }),

  // -------- pty --------
  ptyOpen: (id: string, opts: PtyOpenOpts = {}) => invoke<void>('pty_open', { id, ...opts }),
  ptyOpenSsm: (
    id: string,
    instanceId: string,
    profile?: string,
    region?: string,
    rows?: number,
    cols?: number
  ) => invoke<void>('pty_open_ssm', { id, instanceId, profile, region, rows, cols }),
  ptyOpenEcsExec: (
    id: string,
    cluster: string,
    taskId: string,
    container: string,
    shell?: string,
    profile?: string,
    region?: string,
    rows?: number,
    cols?: number
  ) =>
    invoke<void>('pty_open_ecs_exec', {
      id,
      cluster,
      taskId,
      container,
      shell,
      profile,
      region,
      rows,
      cols
    }),
  ptyWrite: (id: string, data: string) => invoke<void>('pty_write', { id, data }),
  ptyResize: (id: string, rows: number, cols: number) =>
    invoke<void>('pty_resize', { id, rows, cols }),
  ptyClose: (id: string) => invoke<void>('pty_close', { id }),
  onPtyData: (id: string, cb: (chunk: string) => void): Promise<UnlistenFn> =>
    listen<string>(`pty://${id}/data`, (e) => cb(e.payload)),
  onPtyExit: (id: string, cb: () => void): Promise<UnlistenFn> =>
    listen<void>(`pty://${id}/exit`, () => cb())
};

export type Ipc = typeof ipc;
