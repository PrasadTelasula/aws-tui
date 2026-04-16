<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants';

  export const badgeVariants = tv({
    base: 'inline-flex items-center rounded-md border px-2 py-0.5 text-xs font-medium transition-colors',
    variants: {
      variant: {
        default: 'border-transparent bg-primary/10 text-primary',
        secondary: 'border-transparent bg-secondary text-secondary-foreground',
        outline: 'text-foreground',
        ok: 'border-transparent bg-status-ok/15 text-status-ok',
        warn: 'border-transparent bg-status-warn/15 text-status-warn',
        error: 'border-transparent bg-status-error/15 text-status-error',
        info: 'border-transparent bg-status-info/15 text-status-info',
        muted: 'border-transparent bg-muted text-muted-foreground'
      }
    },
    defaultVariants: { variant: 'default' }
  });

  export type BadgeVariant = VariantProps<typeof badgeVariants>['variant'];
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils';

  interface Props extends HTMLAttributes<HTMLSpanElement> {
    variant?: BadgeVariant;
    class?: string;
    children?: Snippet;
  }

  let { variant = 'default', class: className, children, ...rest }: Props = $props();
</script>

<span class={cn(badgeVariants({ variant }), className)} {...rest}>
  {@render children?.()}
</span>
