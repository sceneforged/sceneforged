<script lang="ts">
  import { Select as SelectPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils';
  import { ChevronDown } from 'lucide-svelte';
  import type { Snippet } from 'svelte';

  interface Props extends SelectPrimitive.TriggerProps {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children, ...restProps }: Props = $props();
</script>

<SelectPrimitive.Trigger
  class={cn(
    'flex h-9 w-full items-center justify-between whitespace-nowrap rounded-md border border-[var(--input)] bg-transparent px-3 py-2 text-sm shadow-sm ring-offset-[var(--background)] placeholder:text-[var(--muted-foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--ring)] disabled:cursor-not-allowed disabled:opacity-50 [&>span]:line-clamp-1',
    className
  )}
  {...restProps}
>
  {#if children}
    {@render children()}
  {/if}
  <ChevronDown class="h-4 w-4 opacity-50" />
</SelectPrimitive.Trigger>
