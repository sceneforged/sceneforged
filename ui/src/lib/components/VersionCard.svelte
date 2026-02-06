<script lang="ts">
	import type { MediaFile } from '$lib/types.js';
	import { formatBytes } from '$lib/api/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import ProfileBadge from '$lib/components/ProfileBadge.svelte';
	import { HardDrive, Play, Loader2 } from '@lucide/svelte';

	interface Props {
		mediaFile: MediaFile;
		hasUniversal?: boolean;
		converting?: boolean;
		onConvert?: () => void;
	}

	let { mediaFile, hasUniversal = false, converting = false, onConvert }: Props = $props();

	const roleLabel = $derived(
		mediaFile.role === 'source'
			? 'Source'
			: mediaFile.role === 'universal'
				? 'Universal'
				: mediaFile.role
	);

	const resolution = $derived.by(() => {
		if (!mediaFile.resolution_width || !mediaFile.resolution_height) return null;
		if (mediaFile.resolution_height >= 2160) return '4K';
		if (mediaFile.resolution_height >= 1080) return '1080p';
		if (mediaFile.resolution_height >= 720) return '720p';
		return `${mediaFile.resolution_height}p`;
	});

	const resolutionFull = $derived.by(() => {
		if (!mediaFile.resolution_width || !mediaFile.resolution_height) return null;
		return `${mediaFile.resolution_width}x${mediaFile.resolution_height}`;
	});

	const showConvertButton = $derived(
		mediaFile.role === 'source' && !hasUniversal && onConvert
	);
</script>

<div class="rounded-lg border bg-card p-4">
	<!-- Header with profile badge, role, and size -->
	<div class="mb-3 flex items-center justify-between">
		<div class="flex items-center gap-2">
			<ProfileBadge profile={mediaFile.profile} />
			<Badge variant={mediaFile.profile === 'B' ? 'secondary' : 'default'}>
				{roleLabel}
			</Badge>
			{#if mediaFile.hdr_format}
				<Badge variant="secondary">{mediaFile.hdr_format}</Badge>
			{/if}
			{#if mediaFile.has_dolby_vision}
				<Badge variant="secondary">DV{#if mediaFile.dv_profile} P{mediaFile.dv_profile}{/if}</Badge>
			{/if}
		</div>
		<span class="text-sm font-medium text-muted-foreground">
			{formatBytes(mediaFile.file_size)}
		</span>
	</div>

	<!-- Specs grid -->
	<div class="mb-3 grid grid-cols-2 gap-3 text-sm sm:grid-cols-4">
		{#if resolution}
			<div>
				<span class="text-muted-foreground">Resolution:</span>
				<span class="ml-1 font-medium">{resolution}</span>
				{#if resolutionFull}
					<span class="ml-1 text-xs text-muted-foreground">({resolutionFull})</span>
				{/if}
			</div>
		{/if}

		{#if mediaFile.video_codec}
			<div>
				<span class="text-muted-foreground">Video:</span>
				<span class="ml-1 font-medium uppercase">{mediaFile.video_codec}</span>
			</div>
		{/if}

		{#if mediaFile.audio_codec}
			<div>
				<span class="text-muted-foreground">Audio:</span>
				<span class="ml-1 font-medium uppercase">{mediaFile.audio_codec}</span>
			</div>
		{/if}

		{#if mediaFile.container}
			<div>
				<span class="text-muted-foreground">Container:</span>
				<span class="ml-1 font-medium uppercase">{mediaFile.container}</span>
			</div>
		{/if}
	</div>

	<!-- File path -->
	<div class="mb-3 flex items-center gap-2 truncate text-xs text-muted-foreground/70">
		<HardDrive class="h-3 w-3 flex-shrink-0" />
		<span class="truncate" title={mediaFile.file_path}>{mediaFile.file_path}</span>
	</div>

	<!-- Action area -->
	{#if showConvertButton}
		<div class="border-t pt-3">
			<Button variant="default" size="sm" disabled={converting} onclick={onConvert}>
				{#if converting}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{:else}
					<Play class="mr-2 h-4 w-4" />
				{/if}
				Create Universal Copy
			</Button>
		</div>
	{/if}
</div>
