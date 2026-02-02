<script lang="ts">
  import { DropdownMenu as DropdownMenuPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';

  interface Props extends DropdownMenuPrimitive.GroupHeadingProps {
    class?: string;
    inset?: boolean;
    children?: Snippet;
  }

  let { class: className, inset = false, children, ...restProps }: Props = $props();
</script>

<DropdownMenuPrimitive.GroupHeading
  class={cn('px-2 py-1.5 text-sm font-semibold', inset && 'pl-8', className)}
  {...restProps}
>
  {#if children}
    {@render children()}
  {/if}
</DropdownMenuPrimitive.GroupHeading>
