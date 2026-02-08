<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { getLibrary, getItems, getItem, cancelScan } from '$lib/api/index.js';
	import type { Library, Item, AppEvent, EventPayload } from '$lib/types.js';
	import { MediaGrid } from '$lib/components/media/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Skeleton } from '$lib/components/ui/skeleton/index.js';
	import { eventsService } from '$lib/services/events.svelte.js';
	import { Search, Library as LibraryIcon, Loader2, ArrowLeft, X } from '@lucide/svelte';

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
	let scanProgress = $state<{ phase: string; files_found: number; files_processed: number; files_total: number } | null>(null);

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

	function handleEvent(event: AppEvent): void {
		const { payload } = event;

		if (payload.type === 'item_added' && payload.library_id === libraryId) {
			// Insert a placeholder item directly — no full reload needed
			const existing = items.find((i) => i.id === payload.item_id);
			if (!existing) {
				const placeholder: Item = {
					id: payload.item_id,
					library_id: payload.library_id,
					item_kind: payload.item_kind,
					name: payload.item_name,
					scan_status: 'pending',
					images: [],
					media_files: [],
					created_at: new Date().toISOString(),
					updated_at: new Date().toISOString()
				};
				items = [...items, placeholder];
				totalCount++;
			}
		} else if (payload.type === 'item_status_changed' && payload.library_id === libraryId) {
			// Update item scan status in-place
			const status = payload.scan_status === 'ready' ? undefined : payload.scan_status;
			items = items.map((i) =>
				i.id === payload.item_id ? { ...i, scan_status: status } : i
			);
		} else if (payload.type === 'item_enriched' && payload.library_id === libraryId) {
			// Re-fetch the single item to pick up poster image
			getItem(payload.item_id)
				.then((updated) => {
					items = items.map((i) => (i.id === payload.item_id ? updated : i));
				})
				.catch(() => {});
		} else if (
			payload.type === 'library_scan_started' &&
			payload.library_id === libraryId
		) {
			scanning = true;
			scanProgress = null;
		} else if (payload.type === 'library_scan_progress' && payload.library_id === libraryId) {
			scanning = true;
			scanProgress = {
				phase: payload.phase,
				files_found: payload.files_found,
				files_processed: payload.files_processed,
				files_total: payload.files_total
			};
		} else if (payload.type === 'library_scan_complete' && payload.library_id === libraryId) {
			scanning = false;
			scanProgress = null;
			// Final refresh to reconcile
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

	async function handleCancelScan() {
		try {
			await cancelScan(libraryId);
		} catch {
			// Ignore — scan may have already finished
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
				{#if !loading && totalCount > 0 && !scanning}
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

	<!-- Scan progress banner -->
	{#if scanning}
		<div class="flex items-center gap-3 rounded-lg border border-primary/20 bg-primary/5 px-4 py-3">
			<Loader2 class="h-4 w-4 animate-spin text-primary shrink-0" />
			<div class="flex-1 min-w-0">
				<p class="text-sm font-medium text-foreground">
					Scanning library...
					{#if scanProgress}
						<span class="text-muted-foreground font-normal ml-1">
							{scanProgress.phase}
							{#if scanProgress.files_total > 0}
								&mdash; {scanProgress.files_processed}/{scanProgress.files_total} files
							{:else if scanProgress.files_found > 0}
								&mdash; {scanProgress.files_found} files found
							{/if}
						</span>
					{/if}
				</p>
				{#if scanProgress && scanProgress.files_total > 0}
					<div class="mt-1.5 h-1.5 w-full rounded-full bg-muted overflow-hidden">
						<div
							class="h-full rounded-full bg-primary transition-all duration-300"
							style="width: {Math.round((scanProgress.files_processed / scanProgress.files_total) * 100)}%"
						></div>
					</div>
				{/if}
			</div>
			<Button variant="ghost" size="sm" onclick={handleCancelScan} class="shrink-0">
				<X class="h-4 w-4" />
				Cancel
			</Button>
		</div>
	{/if}

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
