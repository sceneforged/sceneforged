<script lang="ts">
	import type { Item } from '$lib/types';
	import { cn } from '$lib/utils';
	import MediaCard from './MediaCard.svelte';

	interface Props {
		items: Item[];
		libraryId?: string;
		class?: string;
	}

	let { items, libraryId, class: className }: Props = $props();
</script>

<div
	class={cn(
		'grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6',
		className
	)}
>
	{#each items as item (item.id)}
		<MediaCard {item} {libraryId} />
	{/each}
</div>
