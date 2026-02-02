<script lang="ts">
  import { Tabs as TabsPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';

  interface Props extends TabsPrimitive.ListProps {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children, ...restProps }: Props = $props();
</script>

<TabsPrimitive.List
  class={cn(
    'inline-flex h-9 items-center justify-center rounded-lg bg-[var(--muted)] p-1 text-[var(--muted-foreground)]',
    className
  )}
  {...restProps}
>
  {#if children}
    {@render children()}
  {/if}
</TabsPrimitive.List>
