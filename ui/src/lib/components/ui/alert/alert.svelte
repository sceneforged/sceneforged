<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';
  import type { AlertVariant } from './index';

  interface Props extends HTMLAttributes<HTMLDivElement> {
    variant?: AlertVariant;
    class?: string;
    children?: Snippet;
  }

  let {
    variant = 'default',
    class: className,
    children,
    ...restProps
  }: Props = $props();

  const variants: Record<AlertVariant, string> = {
    default: 'bg-[var(--background)] text-[var(--foreground)]',
    destructive: 'border-[var(--destructive)]/50 text-[var(--destructive)] [&>svg]:text-[var(--destructive)]',
  };
</script>

<div
  role="alert"
  class={cn(
    'relative w-full rounded-lg border border-[var(--border)] p-4 [&>svg+div]:translate-y-[-3px] [&>svg]:absolute [&>svg]:left-4 [&>svg]:top-4 [&>svg]:text-[var(--foreground)] [&>svg~*]:pl-7',
    variants[variant],
    className
  )}
  {...restProps}
>
  {#if children}
    {@render children()}
  {/if}
</div>
