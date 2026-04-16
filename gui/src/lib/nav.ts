import type { Component } from 'svelte';
import { Activity, Boxes, Server, TerminalSquare } from 'lucide-svelte';

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
    icon: Activity,
    description: 'Manage SSO, SSM, and IAM sessions'
  },
  {
    href: '/instances',
    label: 'Instances',
    icon: Server,
    description: 'Browse EC2 instances'
  },
  {
    href: '/containers',
    label: 'Containers',
    icon: Boxes,
    description: 'ECS clusters, services, and tasks'
  },
  {
    href: '/terminal',
    label: 'Terminal',
    icon: TerminalSquare,
    description: 'Embedded shell with AWS CLI autocomplete'
  }
];
