<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';

  interface Props {
    class?: string;
    children?: Snippet;
  }

  let { class: className, children }: Props = $props();
</script>

<caption class={cn('mt-4 text-sm text-[var(--muted-foreground)]', className)}>
  {#if children}
    {@render children()}
  {/if}
</caption>
