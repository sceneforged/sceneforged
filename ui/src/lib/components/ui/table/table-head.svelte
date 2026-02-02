<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';
  import type { HTMLThAttributes } from 'svelte/elements';

  interface Props extends HTMLThAttributes {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children, ...restProps }: Props = $props();
</script>

<th
  class={cn(
    'h-10 px-2 text-left align-middle font-medium text-[var(--muted-foreground)] [&:has([role=checkbox])]:pr-0 [&>[role=checkbox]]:translate-y-[2px]',
    className
  )}
  {...restProps}
>
  {#if children}
    {@render children()}
  {/if}
</th>
