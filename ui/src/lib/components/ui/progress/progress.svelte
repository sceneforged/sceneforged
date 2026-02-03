<script lang="ts">
	import { cn } from '$lib/utils';
	import { Progress as ProgressPrimitive } from 'bits-ui';

	interface Props {
		value?: number | null;
		max?: number;
		class?: string;
	}

	let { value = 0, max = 100, class: className }: Props = $props();
</script>

<ProgressPrimitive.Root
	{value}
	{max}
	class={cn('relative h-4 w-full overflow-hidden rounded-full bg-secondary', className)}
>
	<div
		class="h-full bg-primary transition-all"
		style="width: {((value ?? 0) / max) * 100}%"
	></div>
</ProgressPrimitive.Root>
