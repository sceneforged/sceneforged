<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import VersionCard from '$lib/components/VersionCard.svelte';
	import ConversionCard from '$lib/components/ConversionCard.svelte';
	import {
		ArrowLeft,
		Film,
		Tv,
		Music,
		FolderOpen,
		Calendar,
		Clock,
		Star,
		Loader2,
		RefreshCw,
		HardDrive,
		Layers,
		Activity
	} from '@lucide/svelte';
	import * as api from '$lib/api/index.js';
	import { eventsService } from '$lib/services/events.svelte.js';
	import type { Item, MediaFile, ConversionJob, AppEvent } from '$lib/types.js';

	const itemId = $derived($page.params.itemId!);

	let item = $state<Item | null>(null);
	let mediaFiles = $state<MediaFile[]>([]);
	let conversionJobs = $state<ConversionJob[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let converting = $state(false);
	let unsubscribeEvents: (() => void) | null = null;
	let now = $state(Date.now());
	let tickInterval: ReturnType<typeof setInterval> | null = null;

	// Active conversion jobs for this item
	const activeConversionJobs = $derived(
		conversionJobs.filter((j) => j.status === 'queued' || j.status === 'running')
	);
	const hasActiveConversion = $derived(activeConversionJobs.length > 0);

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
			case 'audio_album':
			case 'audio_artist':
				return Music;
			default:
				return FolderOpen;
		}
	});

	// Check if a universal version exists
	const hasUniversal = $derived(mediaFiles.some((f) => f.profile === 'B'));

	// Check if source exists and can create universal
	const canCreateUniversal = $derived.by(() => {
		const sourceFile = mediaFiles.find((f) => f.role === 'source');
		return sourceFile && !hasUniversal;
	});

	async function loadData() {
		if (!itemId) return;
		loading = true;
		error = null;

		try {
			const [itemData, files, cjobs] = await Promise.all([
				api.getItem(itemId),
				api.getItemFiles(itemId).catch((e) => {
					console.error('Failed to load media files:', e);
					return [] as MediaFile[];
				}),
				api.getConversionsForItem(itemId).catch(() => [] as ConversionJob[])
			]);
			item = itemData;
			mediaFiles = files;
			conversionJobs = cjobs;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load item';
		} finally {
			loading = false;
		}
	}

	async function handleConvert() {
		if (!item || converting) return;
		converting = true;

		try {
			await api.convertItem(item.id);
			// Refresh conversion jobs to show the new job
			conversionJobs = await api.getConversionsForItem(itemId).catch(() => []);
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Failed to start conversion';
			console.error(message);
		} finally {
			converting = false;
		}
	}

	async function handleCancelConversion(jobId: string) {
		try {
			await api.deleteConversion(jobId);
			conversionJobs = conversionJobs.filter((j) => j.id !== jobId);
		} catch {
			console.error('Failed to cancel conversion job');
		}
	}

	function handleEvent(event: AppEvent): void {
		const { payload } = event;

		if (payload.type === 'conversion_progress') {
			conversionJobs = conversionJobs.map((j) =>
				j.id === payload.job_id
					? { ...j, progress_pct: payload.progress * 100, status: 'running' }
					: j
			);
		} else if (payload.type === 'conversion_completed') {
			conversionJobs = conversionJobs.filter((j) => j.id !== payload.job_id);
			// Reload media files since a new universal file was registered
			loadData();
		} else if (payload.type === 'conversion_failed') {
			conversionJobs = conversionJobs.map((j) =>
				j.id === payload.job_id
					? { ...j, status: 'failed', error_message: payload.error }
					: j
			);
		} else if (payload.type === 'conversion_queued' || payload.type === 'conversion_started') {
			api.getConversionsForItem(itemId)
				.then((jobs) => {
					conversionJobs = jobs;
				})
				.catch(() => {});
		}
	}

	async function handleRefresh() {
		await loadData();
	}

	onMount(() => {
		loadData();
		unsubscribeEvents = eventsService.subscribe('admin', handleEvent);
		tickInterval = setInterval(() => {
			now = Date.now();
		}, 1000);
	});

	onDestroy(() => {
		if (unsubscribeEvents) {
			unsubscribeEvents();
		}
		if (tickInterval) {
			clearInterval(tickInterval);
		}
	});
</script>

<svelte:head>
	<title>{item?.name ?? 'Item Detail'} - Admin - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header with back button -->
	<div class="flex items-center justify-between">
		<Button variant="ghost" onclick={() => goto('/admin')}>
			<ArrowLeft class="mr-2 h-4 w-4" />
			Back to Admin
		</Button>
		<Button variant="outline" size="sm" onclick={handleRefresh} disabled={loading}>
			<RefreshCw class="mr-2 h-4 w-4 {loading ? 'animate-spin' : ''}" />
			Refresh
		</Button>
	</div>

	{#if loading && !item}
		<div class="flex items-center justify-center py-20">
			<Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
		</div>
	{:else if error || !item}
		<div class="py-20 text-center">
			<p class="mb-4 text-destructive">{error ?? 'Item not found'}</p>
			<Button variant="outline" onclick={() => goto('/admin')}>Return to Admin</Button>
		</div>
	{:else}
		<!-- Item Header -->
		<Card>
			<CardHeader>
				<div class="flex items-start gap-4">
					<!-- Icon placeholder -->
					<div
						class="flex h-16 w-16 flex-shrink-0 items-center justify-center rounded-lg bg-muted"
					>
						{#if ItemIcon === Film}
							<Film class="h-8 w-8 text-muted-foreground" />
						{:else if ItemIcon === Tv}
							<Tv class="h-8 w-8 text-muted-foreground" />
						{:else if ItemIcon === Music}
							<Music class="h-8 w-8 text-muted-foreground" />
						{:else}
							<FolderOpen class="h-8 w-8 text-muted-foreground" />
						{/if}
					</div>

					<div class="min-w-0 flex-1">
						<CardTitle class="mb-2 text-2xl">{item.name}</CardTitle>

						<!-- Metadata badges -->
						<div class="flex flex-wrap gap-2">
							{#if item.year}
								<Badge variant="secondary">
									<Calendar class="mr-1 h-3 w-3" />
									{item.year}
								</Badge>
							{/if}

							{#if item.runtime_minutes}
								<Badge variant="secondary">
									<Clock class="mr-1 h-3 w-3" />
									{api.formatRuntime(item.runtime_minutes)}
								</Badge>
							{/if}

							{#if item.community_rating}
								<Badge variant="secondary">
									<Star class="mr-1 h-3 w-3 fill-yellow-500 text-yellow-500" />
									{item.community_rating.toFixed(1)}
								</Badge>
							{/if}

							<Badge variant="outline" class="capitalize">{item.item_kind}</Badge>
						</div>
					</div>
				</div>
			</CardHeader>

			{#if item.overview}
				<CardContent>
					<p class="leading-relaxed text-muted-foreground">{item.overview}</p>
				</CardContent>
			{/if}
		</Card>

		<!-- Technical Details (from first media file) -->
		{#if mediaFiles.length > 0}
			{@const primaryFile = mediaFiles.find((f) => f.role === 'source') ?? mediaFiles[0]}
			<Card>
				<CardHeader>
					<CardTitle class="flex items-center gap-2 text-lg">
						<HardDrive class="h-5 w-5" />
						Technical Details
					</CardTitle>
				</CardHeader>
				<CardContent>
					<div class="grid grid-cols-2 gap-4 text-sm sm:grid-cols-4">
						{#if primaryFile.resolution_width && primaryFile.resolution_height}
							<div>
								<span class="text-muted-foreground">Resolution:</span>
								<span class="ml-2 font-medium"
									>{primaryFile.resolution_width}x{primaryFile.resolution_height}</span
								>
							</div>
						{/if}
						{#if primaryFile.video_codec}
							<div>
								<span class="text-muted-foreground">Video:</span>
								<span class="ml-2 font-medium uppercase">{primaryFile.video_codec}</span
								>
							</div>
						{/if}
						{#if primaryFile.audio_codec}
							<div>
								<span class="text-muted-foreground">Audio:</span>
								<span class="ml-2 font-medium uppercase">{primaryFile.audio_codec}</span
								>
							</div>
						{/if}
						{#if primaryFile.container}
							<div>
								<span class="text-muted-foreground">Container:</span>
								<span class="ml-2 font-medium uppercase">{primaryFile.container}</span>
							</div>
						{/if}
						<div>
							<span class="text-muted-foreground">Size:</span>
							<span class="ml-2 font-medium"
								>{api.formatBytes(primaryFile.file_size)}</span
							>
						</div>
						{#if primaryFile.hdr_format}
							<div>
								<span class="text-muted-foreground">HDR:</span>
								<span class="ml-2 font-medium">{primaryFile.hdr_format}</span>
							</div>
						{/if}
						{#if primaryFile.has_dolby_vision}
							<div>
								<span class="text-muted-foreground">Dolby Vision:</span>
								<span class="ml-2 font-medium"
									>Profile {primaryFile.dv_profile ?? '?'}</span
								>
							</div>
						{/if}
					</div>

					{#if primaryFile.file_path}
						<div
							class="mt-4 flex items-center gap-2 border-t pt-4 text-xs text-muted-foreground/70"
						>
							<HardDrive class="h-3 w-3 flex-shrink-0" />
							<span class="truncate" title={primaryFile.file_path}
								>{primaryFile.file_path}</span
							>
						</div>
					{/if}
				</CardContent>
			</Card>
		{/if}

		<!-- Active Conversion Jobs -->
		{#if activeConversionJobs.length > 0}
			<Card class="border-blue-500/50">
				<CardHeader>
					<CardTitle class="flex items-center gap-2 text-lg">
						<Activity class="h-5 w-5 animate-pulse text-blue-500" />
						Active Conversion
						<Badge variant="secondary">{activeConversionJobs.length}</Badge>
					</CardTitle>
				</CardHeader>
				<CardContent>
					<div class="space-y-3">
						{#each activeConversionJobs as cjob (cjob.id)}
							<ConversionCard
								job={cjob}
								{now}
								onCancel={handleCancelConversion}
							/>
						{/each}
					</div>
				</CardContent>
			</Card>
		{/if}

		<!-- Versions/Media Files Section -->
		<Card>
			<CardHeader>
				<div class="flex items-center justify-between">
					<CardTitle class="flex items-center gap-2 text-lg">
						<Layers class="h-5 w-5" />
						Versions
						{#if mediaFiles.length > 0}
							<Badge variant="outline">{mediaFiles.length}</Badge>
						{/if}
					</CardTitle>
				</div>
			</CardHeader>
			<CardContent>
				{#if mediaFiles.length === 0}
					<div class="py-8 text-center text-muted-foreground">
						<Layers class="mx-auto mb-2 h-12 w-12 opacity-50" />
						<p>No media files found for this item</p>
					</div>
				{:else}
					<div class="space-y-4">
						{#each mediaFiles as mediaFile (mediaFile.id)}
							<VersionCard
								{mediaFile}
								{hasUniversal}
								{converting}
								onConvert={handleConvert}
							/>
						{/each}
					</div>
				{/if}
			</CardContent>
		</Card>

		<!-- Create Universal Copy action at bottom if applicable -->
		{#if canCreateUniversal && !hasActiveConversion}
			<Card>
				<CardContent class="p-6">
					<div class="flex items-center justify-between">
						<div>
							<h3 class="mb-1 font-semibold">No Universal Version Available</h3>
							<p class="text-sm text-muted-foreground">
								Create a web-playable universal copy for broader device compatibility.
							</p>
						</div>
						<Button variant="default" disabled={converting} onclick={handleConvert}>
							{#if converting}
								<Loader2 class="mr-2 h-4 w-4 animate-spin" />
							{/if}
							Create Universal Copy
						</Button>
					</div>
				</CardContent>
			</Card>
		{/if}
	{/if}
</div>
