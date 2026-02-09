<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { getItem, getItemChildren, submitConversion, getUserData, addFavorite, removeFavorite, searchTmdb, enrichItem, retryProbe } from '$lib/api/index.js';
	import type { Item, AppEvent } from '$lib/types.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import ProgressiveImage from '$lib/components/media/ProgressiveImage.svelte';
	import MediaGrid from '$lib/components/media/MediaGrid.svelte';
	import { conversionsStore } from '$lib/stores/conversions.svelte.js';
	import { authStore } from '$lib/stores/auth.svelte.js';
	import { eventsService } from '$lib/services/events.svelte.js';
	import {
		ArrowLeft,
		Star,
		Clock,
		Calendar,
		Film,
		Tv,
		Music,
		Play,
		Loader2,
		AlertCircle,
		AlertTriangle,
		HardDrive,
		ChevronRight,
		Search as SearchIcon,
		RotateCcw,
		Trash2,
		FileWarning
	} from '@lucide/svelte';

	const libraryId = $derived(page.params.libraryId);
	const itemId = $derived(page.params.itemId);

	let item = $state<Item | null>(null);
	let children = $state<Item[]>([]);
	let parentItem = $state<Item | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Determine if item has a web-playable media file (profile B with HLS ready)
	const isPlayable = $derived(
		item?.media_files?.some((f) => (f.role === 'universal' || f.profile === 'B') && f.hls_ready) ?? false
	);

	// Profile B exists but HLS data not yet prepared
	const isHlsPreparing = $derived(
		!isPlayable &&
		(item?.media_files?.some((f) => (f.role === 'universal' || f.profile === 'B') && !f.hls_ready) ?? false)
	);

	// Is this a container (series/season) that holds children?
	const isContainer = $derived(
		item?.item_kind === 'series' || item?.item_kind === 'season'
	);

	// Seasons for series view
	const seasons = $derived(
		children.filter((c) => c.item_kind === 'season').sort((a, b) => (a.season_number ?? 0) - (b.season_number ?? 0))
	);

	// Episodes for season view
	const episodes = $derived(
		children.filter((c) => c.item_kind === 'episode').sort((a, b) => (a.episode_number ?? 0) - (b.episode_number ?? 0))
	);

	// Favorite state
	let isFavorite = $state(false);
	let togglingFavorite = $state(false);

	// Scan error state
	const isScanError = $derived(item?.scan_status === 'error');
	let retrying = $state(false);
	let retryError = $state<string | null>(null);

	// Conversion state
	let converting = $state(false);
	let convertError = $state<string | null>(null);
	let unsubscribeEvents: (() => void) | null = null;

	const activeConversion = $derived(
		conversionsStore.activeConversions.find((j) => j.item_id === itemId)
	);

	// Refetch item data when a conversion completes (so isPlayable updates)
	let hadActiveConversion = $state(false);

	$effect(() => {
		if (activeConversion) {
			hadActiveConversion = true;
		} else if (hadActiveConversion) {
			hadActiveConversion = false;
			loadItemData();
		}
	});

	// Icon based on item kind
	const ItemIcon = $derived.by(() => {
		if (!item) return Film;
		switch (item.item_kind) {
			case 'movie':
				return Film;
			case 'series':
			case 'season':
			case 'episode':
				return Tv;
			case 'audio':
				return Music;
			default:
				return Film;
		}
	});

	// Get primary poster image
	const posterImage = $derived(item?.images?.find((img) => img.image_type === 'primary'));

	// Get backdrop image
	const backdropImage = $derived(item?.images?.find((img) => img.image_type === 'backdrop'));

	// Format runtime
	const formattedRuntime = $derived.by(() => {
		if (!item?.runtime_minutes) return null;
		const hours = Math.floor(item.runtime_minutes / 60);
		const mins = item.runtime_minutes % 60;
		if (hours > 0) return `${hours}h ${mins}m`;
		return `${mins}m`;
	});

	// Format file size
	function formatBytes(bytes: number): string {
		if (bytes === 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
	}

	onMount(async () => {
		await Promise.all([loadItemData(), conversionsStore.refresh()]);
		unsubscribeEvents = eventsService.subscribe('admin', (event: AppEvent) => {
			conversionsStore.handleEvent(event);
		});
	});

	onDestroy(() => {
		if (unsubscribeEvents) unsubscribeEvents();
	});

	async function loadItemData() {
		if (!itemId) return;
		loading = true;
		error = null;

		try {
			item = await getItem(itemId);

			// Load children for series/season items
			if (item.item_kind === 'series' || item.item_kind === 'season') {
				children = await getItemChildren(itemId);
			} else {
				children = [];
			}

			// Load parent for breadcrumb navigation (season → series, episode → season)
			if (item.parent_id) {
				try {
					parentItem = await getItem(item.parent_id);
				} catch {
					parentItem = null;
				}
			} else {
				parentItem = null;
			}

			// Load user data (playback/favorite state)
			try {
				const userData = await getUserData(itemId);
				isFavorite = userData.is_favorite;
			} catch {
				// Non-critical
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load item';
		} finally {
			loading = false;
		}
	}

	async function toggleFavorite() {
		if (!itemId || togglingFavorite) return;
		togglingFavorite = true;
		try {
			if (isFavorite) {
				await removeFavorite(itemId);
				isFavorite = false;
			} else {
				await addFavorite(itemId);
				isFavorite = true;
			}
		} catch {
			// Silent fail
		} finally {
			togglingFavorite = false;
		}
	}

	async function handleConvert() {
		if (!item || converting) return;
		converting = true;
		convertError = null;
		try {
			await submitConversion({ item_id: item.id });
		} catch (e) {
			convertError = e instanceof Error ? e.message : 'Failed to start conversion';
		} finally {
			converting = false;
		}
	}

	function handlePlay() {
		if (!item || !isPlayable) return;
		goto(`/play/${item.id}`);
	}

	async function handleRetryProbe() {
		if (!item || retrying) return;
		retrying = true;
		retryError = null;
		try {
			const result = await retryProbe(item.id);
			if (result.status === 'ok') {
				await loadItemData();
			} else {
				retryError = result.error ?? 'Probe failed again';
			}
		} catch (e) {
			retryError = e instanceof Error ? e.message : 'Retry failed';
		} finally {
			retrying = false;
		}
	}

	function handleBack() {
		if (parentItem) {
			goto(`/browse/${libraryId}/${parentItem.id}`);
		} else {
			goto(`/browse/${libraryId}`);
		}
	}

	function isEpisodePlayable(ep: Item): boolean {
		return ep.media_files?.some((f) => (f.role === 'universal' || f.profile === 'B') && f.hls_ready) ?? false;
	}

	// TMDB Identify
	let showIdentifyDialog = $state(false);
	let tmdbQuery = $state('');
	let tmdbResults = $state<Array<{ tmdb_id: number; title: string | null; year: string | null; overview: string | null; poster_path: string | null }>>([]);
	let tmdbSearching = $state(false);
	let tmdbEnriching = $state(false);
	let tmdbError = $state<string | null>(null);
	let tmdbSearchTimeout: ReturnType<typeof setTimeout> | null = null;

	const tmdbMediaType = $derived(
		item?.item_kind === 'series' || item?.item_kind === 'season' || item?.item_kind === 'episode' ? 'tv' : 'movie'
	);

	function openIdentify() {
		tmdbQuery = item?.name ?? '';
		tmdbResults = [];
		tmdbError = null;
		showIdentifyDialog = true;
		if (tmdbQuery) doTmdbSearch(tmdbQuery);
	}

	function handleTmdbInput(e: Event) {
		const value = (e.target as HTMLInputElement).value;
		tmdbQuery = value;
		if (tmdbSearchTimeout) clearTimeout(tmdbSearchTimeout);
		if (value.trim().length < 2) {
			tmdbResults = [];
			return;
		}
		tmdbSearchTimeout = setTimeout(() => doTmdbSearch(value.trim()), 400);
	}

	async function doTmdbSearch(q: string) {
		tmdbSearching = true;
		tmdbError = null;
		try {
			const resp = await searchTmdb(q, tmdbMediaType);
			tmdbResults = resp.results;
		} catch (e) {
			tmdbError = e instanceof Error ? e.message : 'Search failed';
		} finally {
			tmdbSearching = false;
		}
	}

	async function selectTmdbResult(tmdbId: number) {
		if (!item || tmdbEnriching) return;
		tmdbEnriching = true;
		tmdbError = null;
		try {
			await enrichItem(item.id, tmdbId, tmdbMediaType);
			showIdentifyDialog = false;
			await loadItemData();
		} catch (e) {
			tmdbError = e instanceof Error ? e.message : 'Enrichment failed';
		} finally {
			tmdbEnriching = false;
		}
	}
</script>

<svelte:head>
	<title>{item?.name ?? 'Loading...'} - SceneForged</title>
</svelte:head>

{#if !loading && !error && item && backdropImage}
	<div class="relative -mx-4 -mt-4 mb-0 h-[300px] overflow-hidden sm:h-[400px] md:-mx-6 md:-mt-6">
		<img
			src="/api/images/{backdropImage.id}?size=large"
			alt=""
			class="absolute inset-0 h-full w-full object-cover"
		/>
		<div
			class="absolute inset-0 bg-gradient-to-t from-background via-background/80 to-transparent"
		></div>
	</div>
{/if}

<div class="relative space-y-6" class:-mt-32={!loading && !error && item && backdropImage}>
	<!-- Breadcrumb navigation -->
	<div class="flex items-center gap-1 text-sm">
		<Button variant="ghost" size="sm" onclick={() => goto(`/browse/${libraryId}`)}>
			<ArrowLeft class="mr-1 h-4 w-4" />
			Back to Library
		</Button>
		{#if parentItem}
			<ChevronRight class="h-4 w-4 text-muted-foreground" />
			<Button variant="ghost" size="sm" onclick={() => goto(`/browse/${libraryId}/${parentItem!.id}`)}>
				{parentItem.name}
			</Button>
		{/if}
		{#if item && (parentItem || item.item_kind !== 'movie')}
			<ChevronRight class="h-4 w-4 text-muted-foreground" />
			<span class="text-muted-foreground">{item.name}</span>
		{/if}
	</div>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
		</div>
	{:else if error || !item}
		<div class="py-20 text-center">
			<AlertCircle class="mx-auto mb-4 h-12 w-12 text-destructive" />
			<p class="text-lg text-destructive">{error ?? 'Item not found'}</p>
			<Button variant="outline" class="mt-4" onclick={handleBack}>
				Return
			</Button>
		</div>
	{:else}
		<!-- Scan error banner -->
		{#if isScanError}
			<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-5">
				<div class="flex items-start gap-3">
					<AlertTriangle class="h-5 w-5 shrink-0 text-destructive mt-0.5" />
					<div class="flex-1 min-w-0">
						<h3 class="font-medium text-destructive">Probe Failed</h3>
						{#if item.scan_error}
							<p class="mt-1 text-sm text-destructive/80">{item.scan_error}</p>
						{/if}
						{#if item.source_file_path}
							<p class="mt-2 text-xs font-mono text-muted-foreground truncate" title={item.source_file_path}>
								{item.source_file_path}
							</p>
						{/if}
						{#if retryError}
							<p class="mt-2 text-sm text-destructive">{retryError}</p>
						{/if}
						<div class="mt-3 flex items-center gap-2">
							<Button variant="outline" size="sm" onclick={handleRetryProbe} disabled={retrying}>
								{#if retrying}
									<Loader2 class="mr-2 h-4 w-4 animate-spin" />
								{:else}
									<RotateCcw class="mr-2 h-4 w-4" />
								{/if}
								Retry Probe
							</Button>
						</div>
					</div>
				</div>
			</div>
		{/if}

		<div class="grid gap-6 md:gap-8 md:grid-cols-3">
			<!-- Poster area -->
			<div class="md:col-span-1">
				<div
					class="mx-auto flex aspect-[2/3] max-w-[280px] items-center justify-center overflow-hidden rounded-lg bg-muted shadow-lg md:max-w-none"
				>
					{#if posterImage}
						<ProgressiveImage
							imageId={posterImage.id}
							alt={item.name}
							size="large"
							class="h-full w-full"
						/>
					{:else}
						<ItemIcon class="h-24 w-24 text-muted-foreground/30" />
					{/if}
				</div>

				<!-- Action buttons (only for playable items, not containers) -->
				{#if !isContainer}
					<div class="mt-6 flex flex-col gap-3">
						{#if isPlayable}
							<Button variant="default" size="lg" class="w-full py-6 text-lg" onclick={handlePlay}>
								<Play class="mr-2 h-6 w-6 fill-current" />
								Play
							</Button>
						{:else if isHlsPreparing}
							<Button variant="secondary" size="lg" class="w-full py-6 text-lg" disabled>
								<Loader2 class="mr-2 h-6 w-6 animate-spin" />
								Preparing HLS...
							</Button>
							<p class="text-center text-sm text-muted-foreground">
								HLS segment data is being prepared for streaming.
							</p>
						{:else if activeConversion}
							<Button variant="secondary" size="lg" class="w-full py-6 text-lg" disabled>
								<Loader2 class="mr-2 h-6 w-6 animate-spin" />
								{activeConversion.status === 'queued' ? 'Queued...' : `Converting ${activeConversion.progress_pct.toFixed(0)}%`}
							</Button>
							{#if activeConversion.status === 'processing'}
								<Progress value={activeConversion.progress_pct} max={100} />
							{/if}
						{:else}
							<Button
								variant="default"
								size="lg"
								class="w-full py-6 text-lg"
								onclick={handleConvert}
								disabled={converting}
							>
								{#if converting}
									<Loader2 class="mr-2 h-6 w-6 animate-spin" />
									Submitting...
								{:else}
									<Film class="mr-2 h-6 w-6" />
									Convert to Profile B
								{/if}
							</Button>
							{#if convertError}
								<p class="text-center text-sm text-destructive">{convertError}</p>
							{:else}
								<p class="text-center text-sm text-muted-foreground">
									Convert this item for web playback.
								</p>
							{/if}
						{/if}
					</div>
				{/if}
			</div>

			<!-- Details section -->
			<div class="md:col-span-2">
				<div class="mb-4 flex items-center gap-3">
					<h1 class="text-2xl font-bold sm:text-3xl">{item.name}</h1>
					{#if authStore.isAdmin}
						<Button variant="ghost" size="sm" onclick={openIdentify} title="Identify with TMDB">
							<SearchIcon class="h-4 w-4" />
						</Button>
					{/if}
					<button
						onclick={toggleFavorite}
						disabled={togglingFavorite}
						class="shrink-0 rounded-full p-1 transition-colors hover:bg-muted"
						aria-label={isFavorite ? 'Remove from favorites' : 'Add to favorites'}
					>
						<Star
							class="h-6 w-6 transition-colors {isFavorite
								? 'fill-yellow-500 text-yellow-500'
								: 'text-muted-foreground'}"
						/>
					</button>
				</div>

				<!-- Metadata badges -->
				<div class="mb-6 flex flex-wrap gap-2">
					{#if item.item_kind === 'series'}
						<Badge variant="secondary">
							<Tv class="mr-1 h-3 w-3" />
							{seasons.length} Season{seasons.length !== 1 ? 's' : ''}
						</Badge>
					{/if}

					{#if item.item_kind === 'season' && item.season_number != null}
						<Badge variant="secondary">
							Season {item.season_number}
						</Badge>
						<Badge variant="secondary">
							{episodes.length} Episode{episodes.length !== 1 ? 's' : ''}
						</Badge>
					{/if}

					{#if item.year}
						<Badge variant="secondary">
							<Calendar class="mr-1 h-3 w-3" />
							{item.year}
						</Badge>
					{/if}

					{#if formattedRuntime}
						<Badge variant="secondary">
							<Clock class="mr-1 h-3 w-3" />
							{formattedRuntime}
						</Badge>
					{/if}

					{#if item.community_rating}
						<Badge variant="secondary">
							<Star class="mr-1 h-3 w-3 fill-yellow-500 text-yellow-500" />
							{item.community_rating.toFixed(1)}
						</Badge>
					{/if}
				</div>

				<!-- Overview -->
				{#if item.overview}
					<div class="mb-6">
						<h2 class="mb-2 text-lg font-semibold">Overview</h2>
						<p class="leading-relaxed text-muted-foreground">{item.overview}</p>
					</div>
				{/if}

				<!-- Seasons grid for series -->
				{#if item.item_kind === 'series' && seasons.length > 0}
					<div class="mb-6">
						<h2 class="mb-3 text-lg font-semibold">Seasons</h2>
						<MediaGrid items={seasons} libraryId={libraryId} />
					</div>
				{/if}

				<!-- Episodes list for season -->
				{#if item.item_kind === 'season' && episodes.length > 0}
					<div class="mb-6">
						<h2 class="mb-3 text-lg font-semibold">Episodes</h2>
						<div class="space-y-2">
							{#each episodes as ep (ep.id)}
								{@const epPlayable = isEpisodePlayable(ep)}
								<button
									type="button"
									class="flex w-full items-center gap-4 rounded-lg border p-3 text-left transition-colors hover:bg-muted/50"
									onclick={() => {
										if (epPlayable) {
											goto(`/play/${ep.id}`);
										} else {
											goto(`/browse/${libraryId}/${ep.id}`);
										}
									}}
								>
									<!-- Episode number -->
									<span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted text-sm font-medium">
										{ep.episode_number ?? '?'}
									</span>

									<!-- Episode poster thumbnail -->
									{#if ep.images?.find((img) => img.image_type === 'primary')}
										{@const epPoster = ep.images.find((img) => img.image_type === 'primary')!}
										<div class="hidden h-16 w-24 shrink-0 overflow-hidden rounded sm:block">
											<ProgressiveImage
												imageId={epPoster.id}
												alt={ep.name}
												size="small"
												class="h-full w-full"
											/>
										</div>
									{/if}

									<!-- Episode info -->
									<div class="min-w-0 flex-1">
										<p class="truncate font-medium">{ep.name}</p>
										<div class="flex items-center gap-2 text-xs text-muted-foreground">
											{#if ep.runtime_minutes}
												<span>{ep.runtime_minutes}m</span>
											{/if}
											{#if ep.community_rating}
												<span class="flex items-center gap-0.5">
													<Star class="h-3 w-3 fill-yellow-500 text-yellow-500" />
													{ep.community_rating.toFixed(1)}
												</span>
											{/if}
										</div>
										{#if ep.overview}
											<p class="mt-1 hidden text-xs leading-relaxed text-muted-foreground line-clamp-2 sm:block">
												{ep.overview}
											</p>
										{/if}
									</div>

									<!-- Play indicator -->
									{#if epPlayable}
										<div class="shrink-0">
											<Play class="h-5 w-5 text-primary" />
										</div>
									{/if}
								</button>
							{/each}
						</div>
					</div>
				{/if}

				<!-- Episode info for TV episodes -->
				{#if item.item_kind === 'episode' && (item.season_number != null || item.episode_number != null)}
					<div class="mb-6">
						<h3 class="mb-1 text-sm font-semibold">Episode Info</h3>
						<div class="text-sm text-muted-foreground">
							{#if item.season_number != null}
								<span>Season {item.season_number}</span>
							{/if}
							{#if item.season_number != null && item.episode_number != null}
								<span> &middot; </span>
							{/if}
							{#if item.episode_number != null}
								<span>Episode {item.episode_number}</span>
							{/if}
						</div>
					</div>
				{/if}

				<!-- Media files -->
				{#if item.media_files && item.media_files.length > 0}
					<div class="mb-6">
						<h2 class="mb-2 text-lg font-semibold">Media Files</h2>
						<div class="space-y-2">
							{#each item.media_files as file}
								<div class="flex items-center justify-between rounded-lg border p-3">
									<div class="flex items-center gap-3">
										<HardDrive class="hidden h-4 w-4 shrink-0 text-muted-foreground sm:block" />
										<div class="min-w-0">
											<p class="truncate text-sm font-medium">{file.file_name}</p>
											<div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
												{#if file.video_codec}
													<span class="uppercase">{file.video_codec}</span>
												{/if}
												{#if file.resolution_width && file.resolution_height}
													<span>{file.resolution_width}x{file.resolution_height}</span>
												{/if}
												{#if file.container}
													<span>.{file.container}</span>
												{/if}
												<span>{formatBytes(file.file_size)}</span>
											</div>
										</div>
									</div>
									<div class="flex shrink-0 gap-1">
										{#if file.profile}
											<Badge variant="secondary" class="text-xs">{file.profile}</Badge>
										{/if}
										{#if file.hdr_format}
											<Badge variant="secondary" class="text-xs bg-amber-600 text-white">
												{file.hdr_format}
											</Badge>
										{/if}
										{#if file.has_dolby_vision}
											<Badge variant="secondary" class="text-xs bg-black text-white">DV</Badge>
										{/if}
									</div>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>

<!-- TMDB Identify Dialog -->
<Dialog.Root bind:open={showIdentifyDialog}>
	<Dialog.Content class="max-w-lg max-h-[80vh]">
		<Dialog.Header>
			<Dialog.Title>Identify with TMDB</Dialog.Title>
			<Dialog.Description>Search for the correct match</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-4">
			<div class="relative">
				<SearchIcon class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
				<Input
					value={tmdbQuery}
					oninput={handleTmdbInput}
					placeholder="Search TMDB..."
					class="pl-9"
				/>
			</div>

			{#if tmdbError}
				<p class="text-sm text-destructive">{tmdbError}</p>
			{/if}

			{#if tmdbSearching}
				<div class="flex justify-center py-4">
					<Loader2 class="h-5 w-5 animate-spin text-muted-foreground" />
				</div>
			{:else if tmdbResults.length > 0}
				<div class="max-h-[50vh] space-y-2 overflow-y-auto">
					{#each tmdbResults as result}
						<button
							type="button"
							class="flex w-full items-start gap-3 rounded-lg border p-3 text-left transition-colors hover:bg-muted/50"
							disabled={tmdbEnriching}
							onclick={() => selectTmdbResult(result.tmdb_id)}
						>
							{#if result.poster_path}
								<img
									src="https://image.tmdb.org/t/p/w92{result.poster_path}"
									alt=""
									class="h-20 w-14 shrink-0 rounded object-cover"
								/>
							{:else}
								<div class="flex h-20 w-14 shrink-0 items-center justify-center rounded bg-muted">
									<Film class="h-6 w-6 text-muted-foreground/30" />
								</div>
							{/if}
							<div class="min-w-0 flex-1">
								<p class="font-medium">
									{result.title ?? 'Unknown'}
									{#if result.year}
										<span class="font-normal text-muted-foreground">({result.year})</span>
									{/if}
								</p>
								{#if result.overview}
									<p class="mt-1 text-xs leading-relaxed text-muted-foreground line-clamp-3">
										{result.overview}
									</p>
								{/if}
							</div>
						</button>
					{/each}
				</div>
			{:else if tmdbQuery.trim().length >= 2}
				<p class="py-4 text-center text-sm text-muted-foreground">No results found</p>
			{/if}

			{#if tmdbEnriching}
				<div class="flex items-center justify-center gap-2 py-2">
					<Loader2 class="h-4 w-4 animate-spin" />
					<span class="text-sm">Applying metadata...</span>
				</div>
			{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
