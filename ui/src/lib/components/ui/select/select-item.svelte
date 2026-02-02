<script lang="ts">
  import { Select as SelectPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils';
  import { Check } from 'lucide-svelte';
  import type { Snippet } from 'svelte';

  interface Props extends SelectPrimitive.ItemProps {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children, ...restProps }: Props = $props();
</script>

<SelectPrimitive.Item
  class={cn(
    'relative flex w-full cursor-default select-none items-center rounded-sm py-1.5 pl-8 pr-2 text-sm outline-none focus:bg-[var(--accent)] focus:text-[var(--accent-foreground)] data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[selected]:font-medium',
    className
  )}
  {...restProps}
>
  <span class="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
    <Check class="h-4 w-4 opacity-0 data-[selected]:opacity-100" />
  </span>
  {#if children}
    {@render children()}
  {/if}
</SelectPrimitive.Item>
