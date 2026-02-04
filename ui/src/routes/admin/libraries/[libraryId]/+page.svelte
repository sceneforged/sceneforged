<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { getLibrary, getItems, scanLibrary } from '$lib/api/index.js';
	import { eventsService } from '$lib/services/events.svelte.js';
	import type { Library, Item } from '$lib/types.js';
	import { MediaGrid } from '$lib/components/media/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import {
		ArrowLeft,
		Library as LibraryIcon,
		Loader2,
		RefreshCw
	} from '@lucide/svelte';

	const libraryId = $derived(page.params.libraryId ?? '');

	let library = $state<Library | null>(null);
	let items = $state<Item[]>([]);
	let totalCount = $state(0);
	let loading = $state(true);
	let loadingMore = $state(false);
	let error = $state<string | null>(null);
	let scanning = $state(false);

	const PAGE_SIZE = 24;
	let currentPage = $state(0);

	const hasMore = $derived(items.length < totalCount);

	let unsubscribe: (() => void) | null = null;

	onMount(() => {
		loadLibraryAndItems();
		unsubscribe = eventsService.subscribe('all', (event) => {
			const { payload } = event;
			if (
				payload.type === 'library_scan_complete' &&
				payload.library_id === libraryId
			) {
				scanning = false;
				loadLibraryAndItems();
			}
		});
	});

	onDestroy(() => {
		unsubscribe?.();
	});

	async function loadLibraryAndItems() {
		loading = true;
		error = null;

		try {
			const [libraryData, itemsData] = await Promise.all([
				getLibrary(libraryId),
				getItems({ library_id: libraryId, limit: PAGE_SIZE, page: 0 })
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

	async function loadMore() {
		if (loadingMore || !hasMore) return;

		loadingMore = true;
		try {
			const nextPage = currentPage + 1;
			const itemsData = await getItems({
				library_id: libraryId,
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

	async function handleScan() {
		scanning = true;
		try {
			await scanLibrary(libraryId);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to start scan';
			scanning = false;
		}
	}
</script>

<svelte:head>
	<title>{library?.name ?? 'Library'} - Admin - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<!-- Back button -->
	<Button variant="ghost" onclick={() => goto('/admin/libraries')}>
		<ArrowLeft class="mr-2 h-4 w-4" />
		Back to Libraries
	</Button>

	<!-- Header -->
	<div class="flex flex-col items-start justify-between gap-4 sm:flex-row sm:items-center">
		<div class="flex items-center gap-3">
			<LibraryIcon class="h-8 w-8 text-primary" />
			<div>
				<h1 class="text-2xl font-bold">{library?.name ?? 'Library'}</h1>
				{#if !loading && totalCount > 0}
					<p class="text-sm text-muted-foreground">
						{totalCount} item{totalCount !== 1 ? 's' : ''}
					</p>
				{/if}
			</div>
		</div>

		<div class="flex items-center gap-2">
			{#if library}
				<Badge variant="outline">{library.media_type}</Badge>
			{/if}
			<Button variant="outline" size="sm" onclick={handleScan} disabled={scanning}>
				{#if scanning}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					Scanning
				{:else}
					Scan
				{/if}
			</Button>
			<Button variant="outline" size="sm" onclick={loadLibraryAndItems} disabled={loading}>
				<RefreshCw class="mr-2 h-4 w-4 {loading ? 'animate-spin' : ''}" />
				Refresh
			</Button>
		</div>
	</div>

	<!-- Library config -->
	{#if library}
		<div class="text-sm text-muted-foreground">
			<span class="font-medium">Paths:</span>
			{#each library.paths as path}
				<code class="ml-2 rounded bg-muted px-2 py-1 text-xs">{path}</code>
			{/each}
		</div>
	{/if}

	<!-- Content -->
	{#if loading && items.length === 0}
		<div class="flex items-center justify-center py-20">
			<Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
		</div>
	{:else if error}
		<div class="py-20 text-center">
			<p class="text-destructive">{error}</p>
			<Button variant="outline" class="mt-4" onclick={loadLibraryAndItems}>Try Again</Button>
		</div>
	{:else if items.length === 0}
		<div class="py-20 text-center">
			<LibraryIcon class="mx-auto mb-4 h-16 w-16 text-muted-foreground/30" />
			<h2 class="text-lg font-medium text-muted-foreground">No items found</h2>
			<p class="mt-1 text-sm text-muted-foreground">This library is empty. Click Scan to discover media files.</p>
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
