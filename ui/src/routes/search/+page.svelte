<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { searchItems } from '$lib/api/index.js';
	import type { Item } from '$lib/types.js';
	import MediaGrid from '$lib/components/media/MediaGrid.svelte';
	import { Search, Loader2 } from '@lucide/svelte';

	let query = $state(page.url.searchParams.get('q') ?? '');
	let results = $state<Item[]>([]);
	let loading = $state(false);
	let searched = $state(false);
	let debounceTimer: ReturnType<typeof setTimeout> | null = null;

	// Run search when query changes from URL
	$effect(() => {
		const urlQuery = page.url.searchParams.get('q') ?? '';
		if (urlQuery && urlQuery !== query) {
			query = urlQuery;
		}
	});

	$effect(() => {
		if (debounceTimer) clearTimeout(debounceTimer);
		if (query.trim().length < 2) {
			results = [];
			searched = false;
			return;
		}
		debounceTimer = setTimeout(() => doSearch(query.trim()), 300);
	});

	async function doSearch(q: string) {
		loading = true;
		try {
			results = await searchItems(q, 50);
		} catch {
			results = [];
		} finally {
			loading = false;
			searched = true;
		}
	}

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement;
		query = target.value;
		// Update URL without navigation
		const url = new URL(window.location.href);
		if (query) {
			url.searchParams.set('q', query);
		} else {
			url.searchParams.delete('q');
		}
		history.replaceState({}, '', url.toString());
	}
</script>

<svelte:head>
	<title>Search{query ? ` - ${query}` : ''} - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<div class="relative">
		<Search class="absolute left-3 top-1/2 h-5 w-5 -translate-y-1/2 text-muted-foreground" />
		<input
			type="text"
			value={query}
			oninput={handleInput}
			placeholder="Search movies, shows, episodes..."
			class="w-full rounded-lg border bg-background py-3 pl-10 pr-4 text-base focus:outline-none focus:ring-2 focus:ring-primary"
			autofocus
		/>
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-12">
			<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
		</div>
	{:else if searched && results.length === 0}
		<p class="py-12 text-center text-muted-foreground">No results found for "{query}"</p>
	{:else if results.length > 0}
		<p class="text-sm text-muted-foreground">{results.length} result{results.length !== 1 ? 's' : ''}</p>
		<MediaGrid items={results} />
	{:else}
		<p class="py-12 text-center text-muted-foreground">Start typing to search your library</p>
	{/if}
</div>
