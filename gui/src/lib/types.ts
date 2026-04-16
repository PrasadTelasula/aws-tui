export type AliasKind = 'sso-login' | 'ssm-session' | 'iam-profile' | 'other';

export interface Alias {
  name: string;
  command: string;
  kind: AliasKind;
  group: string | null;
  subgroup: string | null;
  profile: string | null;
  region: string | null;
  target: string | null;
  ssoSessionName: string | null;
  ssmDocument: string | null;
  ssmLocalPort: string | null;
  ssmRemotePort: string | null;
  ssmHost: string | null;
}

export interface AliasesResponse {
  path: string;
  aliases: Alias[];
}

export interface AppConfig {
  aliasesPath: string | null;
}

export type SessionState =
  | 'stopped'
  | 'starting'
  | 'running'
  | 'connected'
  | 'expired'
  | 'error';

export interface SessionStatus {
  alias: string;
  state: SessionState;
  pid: number | null;
  startedAt: string | null;
  errorMessage: string | null;
  ssoProfile: string | null;
  identityArn: string | null;
  identityAccount: string | null;
  tokenExpiresAt: string | null;
  tokenRemainingSecs: number | null;
  hasCredentials: boolean;
}

export interface CredentialInfo {
  accessKeyId: string;
  secretAccessKey: string;
  sessionToken: string;
  expiration: string;
}

export interface Instance {
  id: string;
  name: string | null;
  state: string;
  instanceType: string;
  privateIp: string | null;
  publicIp: string | null;
  az: string | null;
  vpcId: string | null;
  tags: Record<string, string>;
}

export interface Cluster {
  name: string;
  arn: string;
  status: string;
  runningTasks: number;
  servicesCount: number;
}

export interface Service {
  name: string;
  arn: string;
  cluster: string;
  status: string;
  desired: number;
  running: number;
}

export interface Task {
  arn: string;
  cluster: string;
  service: string | null;
  lastStatus: string;
  desiredStatus: string;
  launchType: string;
}

export interface Container {
  name: string;
  taskArn: string;
  image: string;
  lastStatus: string;
  health: string | null;
}
