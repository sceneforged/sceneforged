<script lang="ts">
	import { onMount } from 'svelte';
	import { getItems, getItem, getContinueWatching, getFavorites } from '$lib/api/index.js';
	import { MediaRow } from '$lib/components/media/index.js';
	import type { Item, PlaybackState, FavoriteState } from '$lib/types.js';
	import { Loader2 } from '@lucide/svelte';

	let recentlyAdded = $state<Item[]>([]);
	let continueWatching = $state<Item[]>([]);
	let favorites = $state<Item[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function resolveItems(ids: string[]): Promise<Item[]> {
		if (ids.length === 0) return [];
		const results = await Promise.allSettled(ids.map((id) => getItem(id)));
		return results
			.filter((r): r is PromiseFulfilledResult<Item> => r.status === 'fulfilled')
			.map((r) => r.value);
	}

	async function loadData() {
		loading = true;
		error = null;

		try {
			const [recentlyAddedRes, continueRes, favsRes] = await Promise.all([
				getItems({ limit: 20 }),
				getContinueWatching(20).catch(() => [] as PlaybackState[]),
				getFavorites(20).catch(() => [] as FavoriteState[])
			]);

			recentlyAdded = recentlyAddedRes.items;

			const [cwItems, favItems] = await Promise.all([
				resolveItems(continueRes.map((p) => p.item_id)),
				resolveItems(favsRes.map((f) => f.item_id))
			]);

			continueWatching = cwItems;
			favorites = favItems;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load data';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		loadData();
	});

	const hasContent = $derived(
		recentlyAdded.length > 0 || continueWatching.length > 0 || favorites.length > 0
	);
</script>

<div class="space-y-8">
	<div>
		<h1 class="text-3xl font-bold text-foreground">Welcome to SceneForged</h1>
		<p class="mt-1 text-muted-foreground">Your personal media library</p>
	</div>

	{#if error}
		<div class="rounded-md bg-destructive/10 p-4 text-destructive">
			{error}
		</div>
	{/if}

	{#if loading}
		<div class="flex items-center justify-center py-12">
			<Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
		</div>
	{:else if !hasContent}
		<div class="py-12 text-center">
			<p class="text-muted-foreground">
				No media to display yet. Add some libraries in the Admin section to get started.
			</p>
		</div>
	{:else}
		<div class="space-y-8">
			{#if continueWatching.length > 0}
				<MediaRow title="Continue Watching" items={continueWatching} />
			{/if}

			{#if favorites.length > 0}
				<MediaRow title="Favorites" items={favorites} />
			{/if}

			{#if recentlyAdded.length > 0}
				<MediaRow title="Recently Added" items={recentlyAdded} />
			{/if}
		</div>
	{/if}
</div>
