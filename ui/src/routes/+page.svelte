<script lang="ts">
	import { onMount } from 'svelte';
	import { getItems, getContinueWatching, getFavorites } from '$lib/api/index.js';
	import { MediaRow } from '$lib/components/media/index.js';
	import type { Item, ContinueWatchingEntry, FavoriteEntry } from '$lib/types.js';
	import { Loader2 } from '@lucide/svelte';
	import { authStore } from '$lib/stores/auth.svelte.js';

	let recentlyAdded = $state<Item[]>([]);
	let continueWatching = $state<Item[]>([]);
	let favorites = $state<Item[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function loadData() {
		loading = true;
		error = null;

		try {
			const [recentlyAddedRes, continueRes, favsRes] = await Promise.all([
				getItems({ limit: 20 }),
				getContinueWatching(20).catch(() => [] as ContinueWatchingEntry[]),
				getFavorites(20).catch(() => [] as FavoriteEntry[])
			]);

			recentlyAdded = recentlyAddedRes.items;
			continueWatching = continueRes.map((e) => e.item);
			favorites = favsRes.map((e) => e.item);
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
				{#if authStore.isAdmin}
					No media to display yet. Add some libraries in the Admin section to get started.
				{:else}
					No media available yet. Ask an administrator to add libraries.
				{/if}
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
