<script lang="ts">
	import type { ConversionJob } from '$lib/types.js';
	import { formatDurationSecs } from '$lib/api/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import { Activity, Clock, XCircle } from '@lucide/svelte';

	interface Props {
		job: ConversionJob;
		now?: number;
		onCancel?: (jobId: string) => void;
	}

	let { job, now = Date.now(), onCancel }: Props = $props();

	// Compute elapsed seconds client-side from started_at timestamp
	const clientElapsed = $derived.by(() => {
		if (!job.started_at || job.status !== 'processing') return job.elapsed_secs ?? null;
		const startMs = new Date(job.started_at).getTime();
		if (isNaN(startMs)) return job.elapsed_secs ?? null;
		return Math.max(0, Math.floor((now - startMs) / 1000));
	});

	// Format codec for display
	function formatCodec(codec: string | null | undefined): string {
		if (!codec) return '?';
		return codec.toUpperCase();
	}

	// Compute target resolution (capped at 1080p, preserving aspect)
	function targetResolution(sourceRes: string | null | undefined): string | null {
		if (!sourceRes) return null;
		const parts = sourceRes.split('x');
		if (parts.length !== 2) return null;
		const sw = parseInt(parts[0]);
		const sh = parseInt(parts[1]);
		if (isNaN(sw) || isNaN(sh)) return null;
		const w = Math.min(sw, 1920);
		const h = Math.min(sh, 1080);
		return `${w}x${h}`;
	}

	const statusBadgeClass = $derived.by(() => {
		switch (job.status) {
			case 'processing':
				return 'bg-blue-500 text-white';
			case 'queued':
				return '';
			case 'completed':
				return 'bg-green-500 text-white';
			case 'failed':
				return 'bg-destructive text-destructive-foreground';
			default:
				return '';
		}
	});
</script>

<div class="space-y-3 rounded-lg border p-4 transition-colors hover:bg-muted/50">
	<div class="flex items-start justify-between">
		<div class="min-w-0 flex-1 space-y-1">
			<h3 class="truncate text-sm font-semibold">
				{job.item_name ?? 'Unknown Item'}
			</h3>
			<div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
				<span>
					{formatCodec(job.source_video_codec)}/{formatCodec(job.source_audio_codec)}
					{#if job.source_resolution}
						({job.source_resolution})
					{/if}
					{#if job.source_container}
						.{job.source_container}
					{/if}
				</span>
				<span class="text-muted-foreground/50">&#8594;</span>
				<span>
					H264/AAC
					{#if targetResolution(job.source_resolution)}
						({targetResolution(job.source_resolution)})
					{/if}
					.mp4
				</span>
			</div>
		</div>
		<div class="ml-2 flex items-center gap-2">
			<Badge
				variant={job.status === 'failed' ? 'destructive' : 'secondary'}
				class={statusBadgeClass}
			>
				{#if job.status === 'processing'}
					<Activity class="mr-1 h-3 w-3 animate-pulse" />
				{:else}
					<Clock class="mr-1 h-3 w-3" />
				{/if}
				{job.status}
			</Badge>
			{#if onCancel && (job.status === 'queued' || job.status === 'processing')}
				<Button
					variant="ghost"
					size="sm"
					class="h-7 w-7 p-0 text-muted-foreground hover:text-destructive"
					onclick={() => onCancel?.(job.id)}
				>
					<XCircle class="h-4 w-4" />
				</Button>
			{/if}
		</div>
	</div>

	{#if job.status === 'processing' || job.progress_pct > 0}
		<div class="space-y-1">
			<div class="flex justify-between text-xs">
				<span class="text-muted-foreground">
					{#if job.encode_fps}
						{job.encode_fps.toFixed(1)} fps
					{:else}
						Encoding...
					{/if}
				</span>
				<span class="font-medium">{job.progress_pct.toFixed(1)}%</span>
			</div>
			<Progress value={job.progress_pct} max={100} />
			<div class="flex justify-between text-xs text-muted-foreground">
				<span>Elapsed: {formatDurationSecs(clientElapsed)}</span>
				{#if job.eta_secs != null && job.eta_secs > 0}
					<span>ETA: {formatDurationSecs(job.eta_secs)}</span>
				{/if}
			</div>
		</div>
	{/if}

	{#if job.error_message || job.error}
		<p class="text-xs text-destructive">{job.error_message ?? job.error}</p>
	{/if}
</div>
