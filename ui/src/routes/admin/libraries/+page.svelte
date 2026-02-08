<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { getLibraries, createLibrary, deleteLibrary, scanLibrary } from '$lib/api/index.js';
	import { eventsService } from '$lib/services/events.svelte.js';
	import type { Library } from '$lib/types.js';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import {
		Library as LibraryIcon,
		Plus,
		Trash2,
		RefreshCw,
		Film,
		Tv,
		Music,
		FolderOpen,
		Loader2,
		ChevronRight,
		Search,
		HardDrive,
		Database,
		Sparkles,
		CheckCircle,
		AlertTriangle,
		ChevronDown
	} from '@lucide/svelte';

	let libraries = $state<Library[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// New library form
	let showForm = $state(false);
	let formName = $state('');
	let formType = $state('movies');
	let formPath = $state('');
	let creating = $state(false);

	// Scan state
	let scanningLibrary = $state<string | null>(null);
	let scanPhase = $state<string>('');
	let scanFilesFound = $state(0);
	let scanFilesQueued = $state(0);
	let scanFilesTotal = $state(0);
	let scanFilesProcessed = $state(0);
	let deletingLibrary = $state<string | null>(null);

	// Discovered items during scan (from item_added events)
	interface DiscoveredItem {
		id: string;
		name: string;
		kind: string;
	}
	let discoveredItems = $state<DiscoveredItem[]>([]);

	// Scan errors
	let scanErrors = $state<{ file_path: string; message: string }[]>([]);
	let showErrors = $state(false);

	// Enriched item tracking
	let enrichedItemIds = $state<Set<string>>(new Set());

	// Scan completion summary
	let scanComplete = $state<{
		files_found: number;
		files_queued: number;
		files_skipped: number;
		errors: number;
	} | null>(null);

	const phaseLabel = $derived.by(() => {
		switch (scanPhase) {
			case 'walking':
				return 'Discovering files';
			case 'probing':
				return 'Analyzing media';
			case 'writing':
				return 'Saving to database';
			case 'enriching':
				return 'Fetching metadata';
			default:
				return 'Starting scan';
		}
	});

	const phaseIcon = $derived.by(() => {
		switch (scanPhase) {
			case 'walking':
				return Search;
			case 'probing':
				return HardDrive;
			case 'writing':
				return Database;
			case 'enriching':
				return Sparkles;
			default:
				return Loader2;
		}
	});

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
				return Tv;
			case 'season':
				return Tv;
			case 'episode':
				return Tv;
			default:
				return Film;
		}
	}

	async function loadLibraries() {
		loading = true;
		error = null;
		try {
			libraries = await getLibraries();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load libraries';
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		if (!formName.trim() || !formPath.trim()) {
			error = 'Name and path are required';
			return;
		}

		creating = true;
		error = null;
		try {
			await createLibrary({
				name: formName.trim(),
				media_type: formType,
				paths: [formPath.trim()]
			});
			await loadLibraries();
			showForm = false;
			formName = '';
			formType = 'movies';
			formPath = '';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to create library';
		} finally {
			creating = false;
		}
	}

	async function handleDelete(lib: Library) {
		if (!confirm(`Delete library "${lib.name}"? This will remove all items from the database.`))
			return;

		deletingLibrary = lib.id;
		try {
			await deleteLibrary(lib.id);
			libraries = libraries.filter((l) => l.id !== lib.id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete library';
		} finally {
			deletingLibrary = null;
		}
	}

	async function handleScan(lib: Library) {
		scanningLibrary = lib.id;
		scanPhase = '';
		scanFilesFound = 0;
		scanFilesQueued = 0;
		scanFilesTotal = 0;
		scanFilesProcessed = 0;
		discoveredItems = [];
		scanErrors = [];
		showErrors = false;
		enrichedItemIds = new Set();
		scanComplete = null;
		try {
			await scanLibrary(lib.id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to start scan';
			scanningLibrary = null;
		}
	}

	function getMediaTypeIcon(type: string) {
		switch (type) {
			case 'movies':
				return Film;
			case 'tvshows':
				return Tv;
			case 'music':
				return Music;
			default:
				return FolderOpen;
		}
	}

	let unsubscribe: (() => void) | null = null;

	onMount(() => {
		loadLibraries();
		unsubscribe = eventsService.subscribe('all', (event) => {
			const { payload } = event;
			if (payload.type === 'library_scan_started') {
				scanningLibrary = payload.library_id;
				scanPhase = 'walking';
				scanFilesFound = 0;
				scanFilesQueued = 0;
				scanFilesTotal = 0;
				scanFilesProcessed = 0;
				discoveredItems = [];
				scanErrors = [];
				showErrors = false;
				enrichedItemIds = new Set();
				scanComplete = null;
			} else if (payload.type === 'library_scan_progress') {
				if (scanningLibrary === payload.library_id) {
					scanFilesFound = payload.files_found;
					scanFilesQueued = payload.files_queued;
					scanPhase = payload.phase;
					scanFilesTotal = payload.files_total;
					scanFilesProcessed = payload.files_processed;
				}
			} else if (payload.type === 'library_scan_complete') {
				if (scanningLibrary === payload.library_id) {
					scanComplete = {
						files_found: payload.files_found,
						files_queued: payload.files_queued,
						files_skipped: payload.files_skipped,
						errors: payload.errors
					};
					scanningLibrary = null;
					scanPhase = '';
				}
				loadLibraries();
			} else if (payload.type === 'item_added') {
				// Add the item tile to the discovered list if it's for our scanning library.
				if (scanningLibrary && payload.library_id === scanningLibrary) {
					discoveredItems = [
						...discoveredItems,
						{
							id: payload.item_id,
							name: payload.item_name,
							kind: payload.item_kind
						}
					];
				}
			} else if (payload.type === 'library_scan_error') {
				if (scanningLibrary && payload.library_id === scanningLibrary) {
					scanErrors = [
						...scanErrors,
						{ file_path: payload.file_path, message: payload.message }
					];
				}
			} else if (payload.type === 'item_enriched') {
				if (payload.library_id === scanningLibrary || scanComplete) {
					enrichedItemIds = new Set([...enrichedItemIds, payload.item_id]);
				}
			} else if (payload.type === 'item_updated') {
				loadLibraries();
			}
		});
	});

	onDestroy(() => {
		unsubscribe?.();
	});
</script>

<svelte:head>
	<title>Libraries - Admin - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-3">
			<LibraryIcon class="h-8 w-8 text-primary" />
			<h1 class="text-2xl font-bold">Libraries</h1>
		</div>
		<div class="flex items-center gap-2">
			<Button variant="outline" size="sm" onclick={loadLibraries} disabled={loading}>
				<RefreshCw class="mr-2 h-4 w-4 {loading ? 'animate-spin' : ''}" />
				Refresh
			</Button>
			<Button size="sm" onclick={() => (showForm = !showForm)}>
				<Plus class="mr-2 h-4 w-4" />
				Add Library
			</Button>
		</div>
	</div>

	{#if error}
		<div class="rounded-md bg-destructive/10 p-4 text-destructive">
			{error}
		</div>
	{/if}

	<!-- Scan Progress Panel -->
	{#if scanningLibrary || scanComplete}
		<Card class="border-blue-500/50">
			<CardHeader class="pb-3">
				<CardTitle class="flex items-center gap-2 text-lg">
					{#if scanningLibrary}
						<Loader2 class="h-5 w-5 animate-spin text-blue-500" />
						Library Scan in Progress
					{:else if scanComplete}
						<CheckCircle class="h-5 w-5 text-green-500" />
						Scan Complete
					{/if}
				</CardTitle>
			</CardHeader>
			<CardContent class="space-y-4">
				{#if scanningLibrary}
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
						<div class="grid max-h-64 grid-cols-2 gap-2 overflow-y-auto sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
							{#each discoveredItems as item (item.id)}
								{@const KindIcon = getItemKindIcon(item.kind)}
								<div
									class="flex items-center gap-2 rounded-md border bg-card p-2"
									style="animation: fadeSlideIn 0.3s ease-out both;"
								>
									<KindIcon class="h-4 w-4 shrink-0 text-muted-foreground" />
									<span class="truncate text-xs font-medium">{item.name}</span>
									{#if enrichedItemIds.has(item.id)}
										<Sparkles class="h-3 w-3 shrink-0 text-amber-500" />
									{/if}
								</div>
							{/each}
						</div>
					</div>
				{/if}

				{#if scanErrors.length > 0}
					<div class="space-y-2">
						<button
							class="flex items-center gap-2 text-xs font-medium uppercase tracking-wider text-destructive"
							onclick={() => (showErrors = !showErrors)}
						>
							<AlertTriangle class="h-3.5 w-3.5" />
							Errors ({scanErrors.length})
							<ChevronDown
								class="h-3.5 w-3.5 transition-transform {showErrors ? 'rotate-180' : ''}"
							/>
						</button>
						{#if showErrors}
							<div class="max-h-48 space-y-1 overflow-y-auto">
								{#each scanErrors as err}
									<div
										class="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-1.5 text-xs"
									>
										<span class="font-mono text-destructive/80">{err.file_path}</span>
										<span class="text-muted-foreground"> — {err.message}</span>
									</div>
								{/each}
							</div>
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
							scanErrors = [];
							showErrors = false;
							enrichedItemIds = new Set();
						}}
					>
						Dismiss
					</Button>
				{/if}
			</CardContent>
		</Card>
	{/if}

	<!-- New Library Form -->
	{#if showForm}
		<Card>
			<CardHeader>
				<CardTitle>New Library</CardTitle>
			</CardHeader>
			<CardContent>
				<div class="space-y-4">
					<div>
						<label for="lib-name" class="text-sm font-medium">Name</label>
						<Input id="lib-name" bind:value={formName} placeholder="My Movies" />
					</div>

					<div>
						<span class="text-sm font-medium">Type</span>
						<div class="mt-2 flex gap-4">
							<label class="flex items-center gap-2">
								<input type="radio" bind:group={formType} value="movies" />
								<Film class="h-4 w-4" />
								Movies
							</label>
							<label class="flex items-center gap-2">
								<input type="radio" bind:group={formType} value="tvshows" />
								<Tv class="h-4 w-4" />
								TV Shows
							</label>
							<label class="flex items-center gap-2">
								<input type="radio" bind:group={formType} value="music" />
								<Music class="h-4 w-4" />
								Music
							</label>
						</div>
					</div>

					<div>
						<label for="lib-path" class="text-sm font-medium">Path</label>
						<Input id="lib-path" bind:value={formPath} placeholder="/media/movies" />
					</div>

					<div class="flex gap-2">
						<Button onclick={handleCreate} disabled={creating}>
							{#if creating}
								<Loader2 class="mr-2 h-4 w-4 animate-spin" />
							{/if}
							Create Library
						</Button>
						<Button variant="outline" onclick={() => (showForm = false)}>Cancel</Button>
					</div>
				</div>
			</CardContent>
		</Card>
	{/if}

	<!-- Libraries List -->
	{#if loading && libraries.length === 0}
		<div class="flex items-center justify-center py-20">
			<Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
		</div>
	{:else if libraries.length === 0}
		<Card>
			<CardContent class="py-12 text-center">
				<LibraryIcon class="mx-auto mb-4 h-16 w-16 text-muted-foreground/30" />
				<h2 class="text-lg font-medium text-muted-foreground">No libraries</h2>
				<p class="mt-1 text-sm text-muted-foreground">
					Add a library to start organizing your media.
				</p>
			</CardContent>
		</Card>
	{:else}
		<div class="grid gap-4">
			{#each libraries as lib (lib.id)}
				{@const Icon = getMediaTypeIcon(lib.media_type)}
				<Card class="transition-colors hover:border-primary/50">
					<CardContent class="p-4">
						<div class="flex items-start justify-between">
							<a
								href="/admin/libraries/{lib.id}"
								class="group flex min-w-0 flex-1 items-start gap-3"
							>
								<div
									class="rounded-lg bg-muted p-2 transition-colors group-hover:bg-primary/10"
								>
									<Icon class="h-6 w-6 text-primary" />
								</div>
								<div class="min-w-0 flex-1">
									<div class="flex items-center gap-2">
										<h3
											class="font-medium transition-colors group-hover:text-primary"
										>
											{lib.name}
										</h3>
										<ChevronRight
											class="h-4 w-4 text-muted-foreground transition-colors group-hover:text-primary"
										/>
									</div>
									<div class="mt-1 flex items-center gap-2">
										<Badge variant="outline">{lib.media_type}</Badge>
									</div>
									<div class="mt-2 text-sm text-muted-foreground">
										{#each lib.paths as path}
											<div class="truncate font-mono text-xs">{path}</div>
										{/each}
									</div>
								</div>
							</a>
							<div class="ml-4 flex items-center gap-2">
								<Button
									variant="outline"
									size="sm"
									onclick={() => handleScan(lib)}
									disabled={scanningLibrary === lib.id}
								>
									{#if scanningLibrary === lib.id}
										<Loader2 class="mr-2 h-4 w-4 animate-spin" />
										Scanning
									{:else}
										Scan
									{/if}
								</Button>
								<Button
									variant="destructive"
									size="sm"
									onclick={() => handleDelete(lib)}
									disabled={deletingLibrary === lib.id}
								>
									<Trash2 class="h-4 w-4" />
								</Button>
							</div>
						</div>
					</CardContent>
				</Card>
			{/each}
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
