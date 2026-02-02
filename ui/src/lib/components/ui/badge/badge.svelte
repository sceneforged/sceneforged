<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';
  import type { BadgeVariant } from './index';

  interface Props extends HTMLAttributes<HTMLDivElement> {
    variant?: BadgeVariant;
    class?: string;
    children?: Snippet;
  }

  let {
    variant = 'default',
    class: className,
    children,
    ...restProps
  }: Props = $props();

  const baseStyles = 'inline-flex items-center rounded-md border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-[var(--ring)] focus:ring-offset-2';

  const variants: Record<BadgeVariant, string> = {
    default: 'border-transparent bg-[var(--primary)] text-[var(--primary-foreground)] shadow hover:bg-[var(--primary)]/80',
    secondary: 'border-transparent bg-[var(--secondary)] text-[var(--secondary-foreground)] hover:bg-[var(--secondary)]/80',
    destructive: 'border-transparent bg-[var(--destructive)] text-[var(--destructive-foreground)] shadow hover:bg-[var(--destructive)]/80',
    outline: 'text-[var(--foreground)]',
  };
</script>

<div class={cn(baseStyles, variants[variant], className)} {...restProps}>
  {#if children}
    {@render children()}
  {/if}
</div>
