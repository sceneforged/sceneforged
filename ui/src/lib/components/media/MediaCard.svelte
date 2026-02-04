<script lang="ts">
	import type { Item } from '$lib/types';
	import { cn } from '$lib/utils';
	import { Film, Tv, Music, FolderOpen, Star, Clock, Play } from '@lucide/svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import ProgressiveImage from './ProgressiveImage.svelte';
	import { goto } from '$app/navigation';

	interface Props {
		item: Item;
		libraryId?: string;
		class?: string;
	}

	let { item, libraryId, class: className }: Props = $props();

	// Resolve library ID from prop or item
	const resolvedLibraryId = $derived(libraryId ?? item.library_id);

	// Determine if item has a web-playable media file (role=universal or profile=B)
	const isWebPlayable = $derived(
		item.media_files?.some((f) => f.role === 'universal' || f.profile === 'B') ?? false
	);

	// Derive profile badge from media files
	const profile = $derived.by(() => {
		if (!item.media_files || item.media_files.length === 0) return null;
		const profiles = new Set(item.media_files.map((f) => f.profile));
		const hasA = profiles.has('A');
		const hasB = profiles.has('B');
		const hasC = profiles.has('C');
		if (hasA && hasB) return 'AB';
		if (hasB) return 'B';
		if (hasA) return 'A';
		if (hasC) return 'C';
		return null;
	});

	// Determine resolution tier from best media file
	const resolutionTier = $derived.by(() => {
		if (!item.media_files || item.media_files.length === 0) return null;
		const maxHeight = Math.max(
			...item.media_files
				.filter((f) => f.resolution_height != null)
				.map((f) => f.resolution_height!)
		);
		if (maxHeight >= 2160) return 'UHD';
		if (maxHeight >= 1080) return 'FHD';
		if (maxHeight >= 720) return 'HD';
		return null;
	});

	// HDR format from best media file
	const hdrFormat = $derived.by(() => {
		if (!item.media_files) return null;
		const hdrFile = item.media_files.find((f) => f.hdr_format);
		return hdrFile?.hdr_format ?? null;
	});

	// Dolby Vision from media files
	const hasDolbyVision = $derived(item.media_files?.some((f) => f.has_dolby_vision) ?? false);

	// Icon based on item kind
	const Icon = $derived.by(() => {
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
				return FolderOpen;
		}
	});

	// Find primary image from item images
	const primaryImage = $derived(item.images?.find((img) => img.image_type === 'primary'));

	// Format runtime
	const formattedRuntime = $derived.by(() => {
		if (!item.runtime_minutes) return null;
		const hours = Math.floor(item.runtime_minutes / 60);
		const mins = item.runtime_minutes % 60;
		if (hours > 0) return `${hours}h ${mins}m`;
		return `${mins}m`;
	});

	// Profile badge styles
	const profileStyles: Record<string, string> = {
		A: 'bg-green-600 text-white',
		B: 'bg-blue-600 text-white',
		AB: 'bg-gradient-to-r from-green-600 to-blue-600 text-white',
		C: 'bg-amber-600 text-white'
	};

	function handlePosterClick(e: MouseEvent) {
		e.stopPropagation();
		if (isWebPlayable) {
			goto(`/play/${item.id}`);
		} else {
			goto(`/browse/${resolvedLibraryId}/${item.id}`);
		}
	}

	function handleTitleClick(e: MouseEvent) {
		e.stopPropagation();
		goto(`/browse/${resolvedLibraryId}/${item.id}`);
	}
</script>

<div
	class={cn(
		'group relative flex flex-col rounded-lg overflow-hidden bg-card shadow-sm hover:shadow-md transition-shadow text-left w-full',
		className
	)}
>
	<!-- Poster/Thumbnail area -->
	<button
		type="button"
		class="relative aspect-[2/3] bg-muted flex items-center justify-center overflow-hidden w-full cursor-pointer"
		onclick={handlePosterClick}
	>
		{#if primaryImage}
			<ProgressiveImage
				imageId={primaryImage.id}
				alt={item.name}
				size="medium"
				class="absolute inset-0 h-full w-full"
			/>
		{:else}
			<Icon class="w-16 h-16 text-muted-foreground/30" />
		{/if}

		<!-- Play overlay on hover (only show if web-playable) -->
		{#if isWebPlayable}
			<div
				class="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center"
			>
				<div class="rounded-full bg-primary p-3">
					<Play class="w-8 h-8 text-white" />
				</div>
			</div>
		{/if}

		<!-- Resolution and HDR/DV badges -->
		<div class="absolute bottom-2 left-2 flex gap-1">
			{#if resolutionTier}
				<Badge
					variant="secondary"
					class={cn(
						'text-xs px-1.5 py-0.5',
						resolutionTier === 'UHD' ? 'bg-purple-600 text-white' : ''
					)}
				>
					{resolutionTier}
				</Badge>
			{/if}
			{#if hdrFormat}
				<Badge variant="secondary" class="text-xs px-1.5 py-0.5 bg-amber-600 text-white">
					{hdrFormat}
				</Badge>
			{/if}
			{#if hasDolbyVision}
				<Badge variant="secondary" class="text-xs px-1.5 py-0.5 bg-black text-white">
					DV
				</Badge>
			{/if}
		</div>

		<!-- Profile badge in top-right corner -->
		{#if profile}
			<div class="absolute top-2 right-2">
				<div
					class={cn(
						'inline-flex items-center justify-center rounded-md px-1.5 py-0.5 text-xs font-semibold',
						profileStyles[profile]
					)}
				>
					{profile}
				</div>
			</div>
		{/if}
	</button>

	<!-- Content -->
	<div class="flex flex-1 flex-col gap-1 p-3">
		<button
			type="button"
			class="cursor-pointer text-left text-sm font-medium line-clamp-2 text-foreground hover:text-primary transition-colors"
			onclick={handleTitleClick}
		>
			{item.name}
		</button>

		<div class="mt-auto flex items-center gap-2 text-xs text-muted-foreground">
			{#if item.year}
				<span>{item.year}</span>
			{/if}

			{#if formattedRuntime}
				<span class="flex items-center gap-1">
					<Clock class="h-3 w-3" />
					{formattedRuntime}
				</span>
			{/if}

			{#if item.community_rating}
				<span class="flex items-center gap-1">
					<Star class="h-3 w-3 fill-yellow-500 text-yellow-500" />
					{item.community_rating.toFixed(1)}
				</span>
			{/if}
		</div>
	</div>
</div>
