<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { getItem, submitConversion, getUserData, addFavorite, removeFavorite } from '$lib/api/index.js';
	import type { Item } from '$lib/types.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import ProgressiveImage from '$lib/components/media/ProgressiveImage.svelte';
	import { conversionsStore } from '$lib/stores/conversions.svelte.js';
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
		HardDrive
	} from '@lucide/svelte';

	const libraryId = $derived(page.params.libraryId);
	const itemId = $derived(page.params.itemId);

	let item = $state<Item | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Determine if item has a web-playable media file
	const isPlayable = $derived(
		item?.media_files?.some((f) => f.role === 'universal' || f.profile === 'B') ?? false
	);

	// Favorite state
	let isFavorite = $state(false);
	let togglingFavorite = $state(false);

	// Conversion state
	let converting = $state(false);
	let convertError = $state<string | null>(null);

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
		await loadItemData();
	});

	async function loadItemData() {
		if (!itemId) return;
		loading = true;
		error = null;

		try {
			item = await getItem(itemId);
			// Load user data (playback/favorite state) in parallel
			try {
				const userData = await getUserData(itemId);
				isFavorite = userData.is_favorite;
			} catch {
				// Non-critical — user data may not exist yet
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

	function handleBack() {
		goto(`/browse/${libraryId}`);
	}
</script>

<svelte:head>
	<title>{item?.name ?? 'Loading...'} - SceneForged</title>
</svelte:head>

{#if !loading && !error && item && backdropImage}
	<div class="relative -mx-4 -mt-4 mb-0 h-[400px] overflow-hidden md:-mx-6 md:-mt-6">
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
	<Button variant="ghost" onclick={handleBack}>
		<ArrowLeft class="mr-2 h-4 w-4" />
		Back to Library
	</Button>

	{#if loading}
		<div class="flex items-center justify-center py-20">
			<Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
		</div>
	{:else if error || !item}
		<div class="py-20 text-center">
			<AlertCircle class="mx-auto mb-4 h-12 w-12 text-destructive" />
			<p class="text-lg text-destructive">{error ?? 'Item not found'}</p>
			<Button variant="outline" class="mt-4" onclick={handleBack}>
				Return to Library
			</Button>
		</div>
	{:else}
		<div class="grid gap-8 md:grid-cols-3">
			<!-- Poster area -->
			<div class="md:col-span-1">
				<div
					class="flex aspect-[2/3] items-center justify-center overflow-hidden rounded-lg bg-muted shadow-lg"
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

				<!-- Action buttons -->
				<div class="mt-6 flex flex-col gap-3">
					{#if isPlayable}
						<Button variant="default" size="lg" class="w-full py-6 text-lg" onclick={handlePlay}>
							<Play class="mr-2 h-6 w-6 fill-current" />
							Play
						</Button>
					{:else if activeConversion}
						<Button variant="secondary" size="lg" class="w-full py-6 text-lg" disabled>
							<Loader2 class="mr-2 h-6 w-6 animate-spin" />
							{activeConversion.status === 'queued' ? 'Queued...' : `Converting ${activeConversion.progress_pct.toFixed(0)}%`}
						</Button>
						{#if activeConversion.status === 'running'}
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
			</div>

			<!-- Details section -->
			<div class="md:col-span-2">
				<div class="mb-4 flex items-center gap-3">
					<h1 class="text-3xl font-bold">{item.name}</h1>
					<button
						onclick={toggleFavorite}
						disabled={togglingFavorite}
						class="rounded-full p-1 transition-colors hover:bg-muted"
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

				<!-- Media files -->
				{#if item.media_files && item.media_files.length > 0}
					<div class="mb-6">
						<h2 class="mb-2 text-lg font-semibold">Media Files</h2>
						<div class="space-y-2">
							{#each item.media_files as file}
								<div class="flex items-center justify-between rounded-lg border p-3">
									<div class="flex items-center gap-3">
										<HardDrive class="h-4 w-4 text-muted-foreground" />
										<div>
											<p class="text-sm font-medium">{file.file_name}</p>
											<div class="flex items-center gap-2 text-xs text-muted-foreground">
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
									<div class="flex gap-1">
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

				<!-- Episode info for TV -->
				{#if item.item_kind === 'episode' && (item.season_number != null || item.episode_number != null)}
					<div class="mb-6">
						<h2 class="mb-2 text-lg font-semibold">Episode Info</h2>
						<div class="text-muted-foreground">
							{#if item.season_number != null}
								<span>Season {item.season_number}</span>
							{/if}
							{#if item.season_number != null && item.episode_number != null}
								<span> - </span>
							{/if}
							{#if item.episode_number != null}
								<span>Episode {item.episode_number}</span>
							{/if}
						</div>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
