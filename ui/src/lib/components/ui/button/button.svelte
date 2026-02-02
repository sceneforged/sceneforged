<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { ButtonVariant, ButtonSize } from './index';

  interface Props extends HTMLButtonAttributes {
    variant?: ButtonVariant;
    size?: ButtonSize;
    class?: string;
    children?: Snippet;
  }

  let {
    variant = 'default',
    size = 'default',
    class: className,
    children,
    ...restProps
  }: Props = $props();

  const baseStyles = 'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring)] disabled:pointer-events-none disabled:opacity-50';

  const variants: Record<ButtonVariant, string> = {
    default: 'bg-[var(--primary)] text-[var(--primary-foreground)] shadow hover:bg-[var(--primary)]/90',
    destructive: 'bg-[var(--destructive)] text-[var(--destructive-foreground)] shadow-sm hover:bg-[var(--destructive)]/90',
    outline: 'border border-[var(--input)] bg-[var(--background)] shadow-sm hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]',
    secondary: 'bg-[var(--secondary)] text-[var(--secondary-foreground)] shadow-sm hover:bg-[var(--secondary)]/80',
    ghost: 'hover:bg-[var(--accent)] hover:text-[var(--accent-foreground)]',
    link: 'text-[var(--primary)] underline-offset-4 hover:underline',
  };

  const sizes: Record<ButtonSize, string> = {
    default: 'h-9 px-4 py-2',
    sm: 'h-8 rounded-md px-3 text-xs',
    lg: 'h-10 rounded-md px-8',
    icon: 'h-9 w-9',
  };
</script>

<button
  class={cn(baseStyles, variants[variant], sizes[size], className)}
  {...restProps}
>
  {#if children}
    {@render children()}
  {/if}
</button>
