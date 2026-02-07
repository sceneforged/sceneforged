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
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import {
		ArrowLeft,
		Library as LibraryIcon,
		Loader2,
		RefreshCw,
		Search,
		HardDrive,
		Database,
		Sparkles,
		CheckCircle,
		Film,
		Tv
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

	// Scan progress state
	let scanPhase = $state<string>('');
	let scanFilesFound = $state(0);
	let scanFilesQueued = $state(0);
	let scanFilesTotal = $state(0);
	let scanFilesProcessed = $state(0);

	interface DiscoveredItem {
		id: string;
		name: string;
		kind: string;
	}
	let discoveredItems = $state<DiscoveredItem[]>([]);

	let scanComplete = $state<{
		files_found: number;
		files_queued: number;
		files_skipped: number;
		errors: number;
	} | null>(null);

	const phaseProgress = $derived.by(() => {
		if (scanFilesTotal === 0) return 0;
		return Math.round((scanFilesProcessed / scanFilesTotal) * 100);
	});

	const phases = ['walking', 'probing', 'writing', 'enriching'] as const;
	const phaseIndex = $derived(phases.indexOf(scanPhase as (typeof phases)[number]));

	function getItemKindIcon(kind: string) {
		switch (kind) {
			case 'movie':
				return Film;
			case 'series':
			case 'season':
			case 'episode':
				return Tv;
			default:
				return Film;
		}
	}

	let unsubscribe: (() => void) | null = null;

	onMount(() => {
		loadLibraryAndItems();
		unsubscribe = eventsService.subscribe('all', (event) => {
			const { payload } = event;
			if (payload.type === 'library_scan_started' && payload.library_id === libraryId) {
				scanning = true;
				scanPhase = 'walking';
				scanFilesFound = 0;
				scanFilesQueued = 0;
				scanFilesTotal = 0;
				scanFilesProcessed = 0;
				discoveredItems = [];
				scanComplete = null;
			} else if (
				payload.type === 'library_scan_progress' &&
				payload.library_id === libraryId
			) {
				scanFilesFound = payload.files_found;
				scanFilesQueued = payload.files_queued;
				scanPhase = payload.phase;
				scanFilesTotal = payload.files_total;
				scanFilesProcessed = payload.files_processed;
			} else if (
				payload.type === 'library_scan_complete' &&
				payload.library_id === libraryId
			) {
				scanComplete = {
					files_found: payload.files_found,
					files_queued: payload.files_queued,
					files_skipped: payload.files_skipped,
					errors: payload.errors
				};
				scanning = false;
				scanPhase = '';
				loadLibraryAndItems();
			} else if (payload.type === 'item_added' && payload.library_id === libraryId) {
				discoveredItems = [
					...discoveredItems,
					{
						id: payload.item_id,
						name: payload.item_name,
						kind: payload.item_kind
					}
				];
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
		scanPhase = '';
		scanFilesFound = 0;
		scanFilesQueued = 0;
		scanFilesTotal = 0;
		scanFilesProcessed = 0;
		discoveredItems = [];
		scanComplete = null;
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

	<!-- Scan Progress Panel -->
	{#if scanning || scanComplete}
		<Card class="border-blue-500/50">
			<CardHeader class="pb-3">
				<CardTitle class="flex items-center gap-2 text-lg">
					{#if scanning}
						<Loader2 class="h-5 w-5 animate-spin text-blue-500" />
						Library Scan in Progress
					{:else if scanComplete}
						<CheckCircle class="h-5 w-5 text-green-500" />
						Scan Complete
					{/if}
				</CardTitle>
			</CardHeader>
			<CardContent class="space-y-4">
				{#if scanning}
					<!-- Phase steps -->
					<div class="flex items-center gap-1">
						{#each phases as phase, i}
							{@const PhIcon = (() => {
								switch (phase) {
									case 'walking':
										return Search;
									case 'probing':
										return HardDrive;
									case 'writing':
										return Database;
									case 'enriching':
										return Sparkles;
								}
							})()}
							<div
								class="flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-medium transition-all {i <= phaseIndex ? 'bg-blue-500/15 text-blue-500' : 'bg-muted text-muted-foreground'} {i === phaseIndex ? 'ring-1 ring-blue-500/50' : ''}"
							>
								<PhIcon class="h-3 w-3" />
								{phase === 'walking'
									? 'Discover'
									: phase === 'probing'
										? 'Analyze'
										: phase === 'writing'
											? 'Save'
											: 'Enrich'}
							</div>
							{#if i < phases.length - 1}
								<div
									class="h-px w-4 {i < phaseIndex ? 'bg-blue-500/50' : 'bg-muted'}"
								></div>
							{/if}
						{/each}
					</div>

					<!-- Phase detail + progress bar -->
					<div class="space-y-2">
						<div class="flex items-center justify-between text-sm">
							<span class="flex items-center gap-2 text-muted-foreground">
								{#if scanPhase === 'walking'}
									Scanning directories...
								{:else if scanPhase === 'probing'}
									Analyzing {scanFilesProcessed} / {scanFilesTotal} files
								{:else if scanPhase === 'writing'}
									Writing batch to database...
								{:else if scanPhase === 'enriching'}
									Fetching metadata from TMDB...
								{:else}
									Initializing...
								{/if}
							</span>
							<span class="font-medium tabular-nums">
								{scanFilesFound} found
							</span>
						</div>
						{#if scanPhase === 'probing' && scanFilesTotal > 0}
							<Progress value={phaseProgress} max={100} />
						{:else}
							<div class="h-2 w-full overflow-hidden rounded-full bg-muted">
								<div
									class="h-full animate-pulse rounded-full bg-blue-500/50"
									style="width: 100%"
								></div>
							</div>
						{/if}
					</div>
				{/if}

				{#if scanComplete}
					<div class="grid grid-cols-4 gap-4 text-center text-sm">
						<div>
							<div class="text-2xl font-bold tabular-nums">{scanComplete.files_found}</div>
							<div class="text-muted-foreground">Files Found</div>
						</div>
						<div>
							<div class="text-2xl font-bold tabular-nums">{discoveredItems.length}</div>
							<div class="text-muted-foreground">Items Added</div>
						</div>
						<div>
							<div class="text-2xl font-bold tabular-nums">{scanComplete.files_skipped}</div>
							<div class="text-muted-foreground">Skipped</div>
						</div>
						<div>
							<div class="text-2xl font-bold tabular-nums {scanComplete.errors > 0 ? 'text-destructive' : ''}">
								{scanComplete.errors}
							</div>
							<div class="text-muted-foreground">Errors</div>
						</div>
					</div>
				{/if}

				<!-- Discovered item tiles -->
				{#if discoveredItems.length > 0}
					<div class="space-y-2">
						<h4 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">
							Discovered Items ({discoveredItems.length})
						</h4>
						<div class="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
							{#each discoveredItems.slice(-20) as item (item.id)}
								{@const KindIcon = getItemKindIcon(item.kind)}
								<div
									class="flex items-center gap-2 rounded-md border bg-card p-2 opacity-0 animate-in fade-in"
									style="animation: fadeSlideIn 0.3s ease-out forwards;"
								>
									<KindIcon class="h-4 w-4 shrink-0 text-muted-foreground" />
									<span class="truncate text-xs font-medium">{item.name}</span>
								</div>
							{/each}
						</div>
						{#if discoveredItems.length > 20}
							<p class="text-xs text-muted-foreground">
								...and {discoveredItems.length - 20} more
							</p>
						{/if}
					</div>
				{/if}

				{#if scanComplete}
					<Button
						variant="outline"
						size="sm"
						onclick={() => {
							scanComplete = null;
							discoveredItems = [];
						}}
					>
						Dismiss
					</Button>
				{/if}
			</CardContent>
		</Card>
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

<style>
	@keyframes fadeSlideIn {
		from {
			opacity: 0;
			transform: translateY(4px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
</style>
