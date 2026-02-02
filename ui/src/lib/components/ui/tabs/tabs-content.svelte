<script lang="ts">
  import { Tabs as TabsPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';

  interface Props extends TabsPrimitive.ContentProps {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children, ...restProps }: Props = $props();
</script>

<TabsPrimitive.Content
  class={cn(
    'mt-2 ring-offset-[var(--background)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] focus-visible:ring-offset-2',
    className
  )}
  {...restProps}
>
  {#if children}
    {@render children()}
  {/if}
</TabsPrimitive.Content>
