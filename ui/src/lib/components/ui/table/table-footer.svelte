<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';

  interface Props extends HTMLAttributes<HTMLTableSectionElement> {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children, ...restProps }: Props = $props();
</script>

<tfoot
  class={cn('border-t bg-[var(--muted)]/50 font-medium [&>tr]:last:border-b-0', className)}
  {...restProps}
>
  {#if children}
    {@render children()}
  {/if}
</tfoot>
