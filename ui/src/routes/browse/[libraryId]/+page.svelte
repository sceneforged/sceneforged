<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { getLibrary, getItems } from '$lib/api/index.js';
	import type { Library, Item, AppEvent } from '$lib/types.js';
	import { MediaGrid } from '$lib/components/media/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Skeleton } from '$lib/components/ui/skeleton/index.js';
	import { eventsService } from '$lib/services/events.svelte.js';
	import { Search, Library as LibraryIcon, Loader2, ArrowLeft } from '@lucide/svelte';

	const libraryId = $derived(page.params.libraryId ?? '');

	let library = $state<Library | null>(null);
	let items = $state<Item[]>([]);
	let totalCount = $state(0);
	let loading = $state(true);
	let loadingMore = $state(false);
	let error = $state<string | null>(null);
	let searchQuery = $state('');
	let searchTimeout: ReturnType<typeof setTimeout> | null = null;
	let initialLoadDone = $state(false);
	let unsubscribeEvents: (() => void) | null = null;
	let refreshTimeout: ReturnType<typeof setTimeout> | null = null;
	let scanning = $state(false);

	const PAGE_SIZE = 24;
	let currentPage = $state(0);

	const hasMore = $derived(items.length < totalCount);

	$effect(() => {
		const query = searchQuery;
		if (!initialLoadDone) return;

		if (searchTimeout) clearTimeout(searchTimeout);
		searchTimeout = setTimeout(() => {
			currentPage = 0;
			items = [];
			loadItems(query);
		}, 300);
	});

	function debouncedRefresh() {
		if (refreshTimeout) clearTimeout(refreshTimeout);
		refreshTimeout = setTimeout(() => {
			loadItems(searchQuery || undefined);
		}, 2000);
	}

	function handleEvent(event: AppEvent): void {
		const { payload } = event;
		if (payload.type === 'item_added') {
			debouncedRefresh();
		} else if (payload.type === 'library_scan_started' && 'library_id' in payload && payload.library_id === libraryId) {
			scanning = true;
		} else if (payload.type === 'library_scan_progress' && payload.library_id === libraryId) {
			scanning = true;
			debouncedRefresh();
		} else if (payload.type === 'library_scan_complete' && payload.library_id === libraryId) {
			scanning = false;
			if (refreshTimeout) clearTimeout(refreshTimeout);
			loadItems(searchQuery || undefined);
		}
	}

	onMount(async () => {
		await loadLibraryAndItems();
		initialLoadDone = true;
		unsubscribeEvents = eventsService.subscribe('user', handleEvent);
	});

	onDestroy(() => {
		if (unsubscribeEvents) unsubscribeEvents();
		if (refreshTimeout) clearTimeout(refreshTimeout);
	});

	async function loadLibraryAndItems() {
		loading = true;
		error = null;

		try {
			const [libraryData, itemsData] = await Promise.all([
				getLibrary(libraryId),
				getItems({
					library_id: libraryId,
					limit: PAGE_SIZE,
					page: 0
				})
			]);

			library = libraryData;
			items = itemsData.items;
			totalCount = itemsData.total;
			currentPage = 0;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load library';
		} finally {
			loading = false;
		}
	}

	async function loadItems(query?: string) {
		loading = items.length === 0;
		error = null;

		try {
			const itemsData = await getItems({
				library_id: libraryId,
				search: query || undefined,
				limit: PAGE_SIZE,
				page: 0
			});

			items = itemsData.items;
			totalCount = itemsData.total;
			currentPage = 0;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load items';
		} finally {
			loading = false;
		}
	}

	async function loadMore() {
		if (loadingMore || !hasMore) return;

		loadingMore = true;

		try {
			const nextPage = currentPage + 1;
			const itemsData = await getItems({
				library_id: libraryId,
				search: searchQuery || undefined,
				limit: PAGE_SIZE,
				page: nextPage
			});

			items = [...items, ...itemsData.items];
			currentPage = nextPage;
			totalCount = itemsData.total;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load more items';
		} finally {
			loadingMore = false;
		}
	}
</script>

<svelte:head>
	<title>{library?.name ?? 'Browse'} - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<!-- Back button -->
	<Button variant="ghost" onclick={() => goto('/')}>
		<ArrowLeft class="mr-2 h-4 w-4" />
		Back to Home
	</Button>

	<!-- Header -->
	<div class="flex flex-col items-start justify-between gap-4 sm:flex-row sm:items-center">
		<div class="flex items-center gap-3">
			<LibraryIcon class="h-8 w-8 text-primary" />
			<div>
				<h1 class="text-2xl font-bold">{library?.name ?? 'Browse'}</h1>
				{#if scanning}
					<p class="flex items-center gap-1 text-sm text-muted-foreground">
						<Loader2 class="h-3 w-3 animate-spin" />
						Scanning... {totalCount} item{totalCount !== 1 ? 's' : ''}
					</p>
				{:else if !loading && totalCount > 0}
					<p class="text-sm text-muted-foreground">
						{totalCount} item{totalCount !== 1 ? 's' : ''}
					</p>
				{/if}
			</div>
		</div>

		<!-- Search -->
		<div class="relative w-full sm:w-64">
			<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
			<Input type="search" placeholder="Search..." class="pl-9" bind:value={searchQuery} />
		</div>
	</div>

	<!-- Content -->
	{#if loading && items.length === 0}
		<div class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6">
			{#each Array(12) as _}
				<Skeleton class="aspect-[2/3] rounded-lg" />
			{/each}
		</div>
	{:else if error}
		<div class="py-20 text-center">
			<p class="text-destructive">{error}</p>
			<Button variant="outline" class="mt-4" onclick={() => loadLibraryAndItems()}>
				Try Again
			</Button>
		</div>
	{:else if items.length === 0}
		<div class="py-20 text-center">
			<LibraryIcon class="mx-auto mb-4 h-16 w-16 text-muted-foreground/30" />
			<h2 class="text-lg font-medium text-muted-foreground">No items found</h2>
			<p class="mt-1 text-sm text-muted-foreground">
				{searchQuery ? 'Try a different search term' : 'This library is empty'}
			</p>
		</div>
	{:else}
		<MediaGrid {items} libraryId={libraryId} />

		{#if hasMore}
			<div class="mt-8 flex justify-center">
				<Button variant="outline" disabled={loadingMore} onclick={loadMore}>
					{#if loadingMore}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					Load More
				</Button>
			</div>
		{/if}

		<div class="mt-4 text-center text-sm text-muted-foreground">
			Showing {items.length} of {totalCount} item{totalCount !== 1 ? 's' : ''}
		</div>
	{/if}
</div>
