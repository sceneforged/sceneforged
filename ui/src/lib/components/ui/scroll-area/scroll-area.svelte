<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';

  interface Props extends HTMLAttributes<HTMLDivElement> {
    orientation?: 'vertical' | 'horizontal' | 'both';
    class?: string;
    children?: Snippet;
  }

  let {
    orientation = 'vertical',
    class: className,
    children,
    ...restProps
  }: Props = $props();
</script>

<div
  class={cn(
    'relative overflow-hidden',
    className
  )}
  {...restProps}
>
  <div
    class={cn(
      'h-full w-full',
      orientation === 'vertical' && 'overflow-y-auto overflow-x-hidden',
      orientation === 'horizontal' && 'overflow-x-auto overflow-y-hidden',
      orientation === 'both' && 'overflow-auto'
    )}
    style="scrollbar-width: thin; scrollbar-color: var(--muted) transparent;"
  >
    {#if children}
      {@render children()}
    {/if}
  </div>
</div>

<style>
  div :global(::-webkit-scrollbar) {
    width: 8px;
    height: 8px;
  }

  div :global(::-webkit-scrollbar-track) {
    background: transparent;
  }

  div :global(::-webkit-scrollbar-thumb) {
    background-color: var(--muted);
    border-radius: 4px;
  }

  div :global(::-webkit-scrollbar-thumb:hover) {
    background-color: var(--muted-foreground);
  }
</style>
