<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { getItem, getPlayback, updateProgress, markPlayed } from '$lib/api/index.js';
	import type { Item } from '$lib/types.js';
	import { VideoPlayer } from '$lib/components/media/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { ArrowLeft, Loader2, AlertCircle } from '@lucide/svelte';

	const itemId = $derived(page.params.itemId);

	let item = $state<Item | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let streamUrl = $state<string | null>(null);
	let startPosition = $state(0);

	onMount(async () => {
		await loadPlaybackInfo();
	});

	async function loadPlaybackInfo() {
		if (!itemId) return;
		loading = true;
		error = null;

		try {
			item = await getItem(itemId);

			// Find the web-playable media file (universal/profile B)
			const playableFile = item.media_files?.find(
				(f) => f.role === 'universal' || f.profile === 'B'
			);

			if (!playableFile) {
				error = 'No playable source found for this item';
				return;
			}

			// Construct HLS stream URL
			streamUrl = `/api/stream/${playableFile.id}/index.m3u8`;

			// Restore playback position
			try {
				const pb = await getPlayback(itemId);
				if (pb && !pb.completed && pb.position_secs > 5) {
					startPosition = pb.position_secs;
				}
			} catch {
				// No playback state yet — start from beginning
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load playback info';
		} finally {
			loading = false;
		}
	}

	function handleProgress(positionSeconds: number) {
		if (!itemId) return;
		updateProgress(itemId, positionSeconds).catch(() => {
			// Silent fail — don't interrupt playback for progress saves
		});
	}

	function handleEnded() {
		if (itemId) {
			markPlayed(itemId).catch(() => {});
		}
		goBack();
	}

	function handleError(errorMessage: string) {
		error = errorMessage;
	}

	function goBack() {
		if (item) {
			goto(`/browse/${item.library_id}/${itemId}`);
		} else {
			goto('/');
		}
	}
</script>

<svelte:head>
	<title>{item?.name ?? 'Playing'} - SceneForged</title>
</svelte:head>

<div class="flex min-h-screen flex-col bg-black">
	<!-- Header -->
	<div
		class="absolute left-0 right-0 top-0 z-10 bg-gradient-to-b from-black/80 to-transparent p-4"
	>
		<div class="flex items-center gap-4">
			<Button variant="ghost" size="icon" class="text-white hover:bg-white/20" onclick={goBack}>
				<ArrowLeft class="h-5 w-5" />
			</Button>
			{#if item}
				<div class="text-white">
					<h1 class="font-medium">{item.name}</h1>
					{#if item.year}
						<p class="text-sm text-white/70">{item.year}</p>
					{/if}
				</div>
			{/if}
		</div>
	</div>

	<!-- Main content -->
	<div class="flex flex-1 items-center justify-center">
		{#if loading}
			<div class="flex flex-col items-center gap-4 text-white">
				<Loader2 class="h-12 w-12 animate-spin" />
				<p>Loading...</p>
			</div>
		{:else if error}
			<div class="flex max-w-md flex-col items-center gap-4 px-4 text-center text-white">
				<AlertCircle class="h-12 w-12 text-destructive" />
				<h2 class="text-xl font-medium">Playback Error</h2>
				<p class="text-white/70">{error}</p>
				<div class="mt-4 flex gap-4">
					<Button variant="outline" onclick={goBack}>
						<ArrowLeft class="mr-2 h-4 w-4" />
						Go Back
					</Button>
					<Button onclick={loadPlaybackInfo}>Try Again</Button>
				</div>
			</div>
		{:else if streamUrl}
			<div class="h-full w-full max-w-screen-2xl">
				<VideoPlayer
					src={streamUrl}
					title={item?.name}
					{startPosition}
					onProgress={handleProgress}
					onEnded={handleEnded}
					onError={handleError}
				/>
			</div>
		{/if}
	</div>
</div>

<style>
	:global(body) {
		overflow: hidden;
	}
</style>
