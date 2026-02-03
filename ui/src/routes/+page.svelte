<script lang="ts">
	import { onMount } from 'svelte';
	import { getItems } from '$lib/api/index.js';
	import { MediaRow } from '$lib/components/media/index.js';
	import type { Item } from '$lib/types.js';
	import { Loader2 } from 'lucide-svelte';

	let recentlyAdded = $state<Item[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function loadData() {
		loading = true;
		error = null;

		try {
			const recentlyAddedRes = await getItems({ limit: 20 });
			recentlyAdded = recentlyAddedRes.items;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load data';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		loadData();
	});

	const hasContent = $derived(recentlyAdded.length > 0);
</script>

<div class="space-y-8">
	<div>
		<h1 class="text-3xl font-bold text-foreground">Welcome to SceneForged</h1>
		<p class="mt-1 text-muted">Your personal media library</p>
	</div>

	{#if error}
		<div class="rounded-md bg-destructive/10 p-4 text-destructive">
			{error}
		</div>
	{/if}

	{#if loading}
		<div class="flex items-center justify-center py-12">
			<Loader2 class="h-8 w-8 animate-spin text-muted" />
		</div>
	{:else if !hasContent}
		<div class="py-12 text-center">
			<p class="text-muted">
				No media to display yet. Add some libraries in the Admin section to get started.
			</p>
		</div>
	{:else}
		<div class="space-y-8">
			{#if recentlyAdded.length > 0}
				<MediaRow title="Recently Added" items={recentlyAdded} />
			{/if}
		</div>
	{/if}
</div>
