<script lang="ts">
  import { cn } from '$lib/utils';
  import type { HTMLAttributes } from 'svelte/elements';

  interface Props extends HTMLAttributes<HTMLDivElement> {
    value?: number;
    max?: number;
    class?: string;
  }

  let { value = 0, max = 100, class: className, ...restProps }: Props = $props();

  $effect(() => {
    if (value < 0) value = 0;
    if (value > max) value = max;
  });
</script>

<div
  role="progressbar"
  aria-valuemin={0}
  aria-valuemax={max}
  aria-valuenow={value}
  class={cn(
    'relative h-2 w-full overflow-hidden rounded-full bg-[var(--primary)]/20',
    className
  )}
  {...restProps}
>
  <div
    class="h-full w-full flex-1 bg-[var(--primary)] transition-all"
    style="transform: translateX(-{100 - (value / max) * 100}%)"
  ></div>
</div>
