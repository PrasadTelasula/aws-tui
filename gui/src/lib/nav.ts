import type { Component } from 'svelte';
import { Pulse, Stack, HardDrives, TerminalWindow } from 'phosphor-svelte';

export interface NavEntry {
  href: string;
  label: string;
  icon: Component;
  description: string;
}

export const navEntries: NavEntry[] = [
  {
    href: '/',
    label: 'Sessions',
    icon: Pulse,
    description: 'Manage SSO, SSM, and IAM sessions'
  },
  {
    href: '/instances',
    label: 'Instances',
    icon: HardDrives,
    description: 'Browse EC2 instances'
  },
  {
    href: '/containers',
    label: 'Containers',
    icon: Stack,
    description: 'ECS clusters, services, and tasks'
  },
  {
    href: '/terminal',
    label: 'Terminal',
    icon: TerminalWindow,
    description: 'Embedded shell with AWS CLI autocomplete'
  }
];
