<script lang="ts">
  import { DropdownMenu as DropdownMenuPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils';

  interface Props extends DropdownMenuPrimitive.SeparatorProps {
    class?: string;
  }

  let { class: className, ...restProps }: Props = $props();
</script>

<DropdownMenuPrimitive.Separator
  class={cn('-mx-1 my-1 h-px bg-[var(--muted)]', className)}
  {...restProps}
/>
