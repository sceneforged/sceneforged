<script lang="ts">
	import type { Item } from '$lib/types';
	import MediaCard from './MediaCard.svelte';

	interface Props {
		title: string;
		items: Item[];
	}

	let { title, items }: Props = $props();
</script>

<section class="space-y-4">
	<h2 class="text-xl font-semibold text-foreground">{title}</h2>

	<div class="relative">
		<div class="scrollbar-thin flex gap-4 overflow-x-auto pb-4">
			{#each items as item (item.id)}
				<div class="w-40 flex-shrink-0">
					<MediaCard {item} />
				</div>
			{/each}
		</div>
	</div>
</section>

<style>
	.scrollbar-thin::-webkit-scrollbar {
		height: 8px;
	}

	.scrollbar-thin::-webkit-scrollbar-track {
		background: transparent;
	}

	.scrollbar-thin::-webkit-scrollbar-thumb {
		background-color: var(--color-muted);
		border-radius: 4px;
	}

	.scrollbar-thin::-webkit-scrollbar-thumb:hover {
		background-color: var(--color-foreground);
	}
</style>
