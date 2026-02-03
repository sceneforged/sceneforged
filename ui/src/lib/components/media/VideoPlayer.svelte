<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import Hls from 'hls.js';
	import {
		Play,
		Pause,
		Volume2,
		VolumeX,
		Maximize,
		Minimize,
		SkipBack,
		SkipForward
	} from 'lucide-svelte';

	interface Props {
		src: string;
		poster?: string;
		title?: string;
		startPosition?: number;
		onProgress?: (positionSeconds: number) => void;
		onEnded?: () => void;
		onError?: (error: string) => void;
	}

	let {
		src,
		poster,
		title,
		startPosition = 0,
		onProgress,
		onEnded,
		onError
	}: Props = $props();

	let videoElement: HTMLVideoElement;
	let containerElement: HTMLDivElement;
	let hls: Hls | null = null;

	let isPlaying = $state(false);
	let isMuted = $state(false);
	let isFullscreen = $state(false);
	let showControls = $state(true);
	let currentTime = $state(0);
	let duration = $state(0);
	let buffered = $state(0);
	let volume = $state(1);
	let controlsTimeout: ReturnType<typeof setTimeout> | null = null;
	let progressInterval: ReturnType<typeof setInterval> | null = null;
	let lastReportedPosition = 0;

	// Format time display
	function formatTime(seconds: number): string {
		const h = Math.floor(seconds / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		const s = Math.floor(seconds % 60);
		if (h > 0) {
			return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
		}
		return `${m}:${s.toString().padStart(2, '0')}`;
	}

	const progressPercent = $derived(duration > 0 ? (currentTime / duration) * 100 : 0);
	const bufferedPercent = $derived(duration > 0 ? (buffered / duration) * 100 : 0);

	onMount(() => {
		if (!videoElement) return;

		if (Hls.isSupported()) {
			hls = new Hls({
				enableWorker: true,
				lowLatencyMode: false,
				backBufferLength: 90
			});

			hls.loadSource(src);
			hls.attachMedia(videoElement);

			hls.on(Hls.Events.MANIFEST_PARSED, () => {
				if (startPosition > 0) {
					videoElement.currentTime = startPosition;
				}
			});

			hls.on(Hls.Events.ERROR, (_event, data) => {
				if (data.fatal) {
					switch (data.type) {
						case Hls.ErrorTypes.NETWORK_ERROR:
							onError?.('Network error occurred');
							hls?.startLoad();
							break;
						case Hls.ErrorTypes.MEDIA_ERROR:
							onError?.('Media error occurred');
							hls?.recoverMediaError();
							break;
						default:
							onError?.(`Fatal error: ${data.details}`);
							hls?.destroy();
							break;
					}
				}
			});
		} else if (videoElement.canPlayType('application/vnd.apple.mpegurl')) {
			videoElement.src = src;
			if (startPosition > 0) {
				videoElement.currentTime = startPosition;
			}
		} else {
			onError?.('HLS is not supported in this browser');
		}

		progressInterval = setInterval(() => {
			if (isPlaying && videoElement && Math.abs(currentTime - lastReportedPosition) > 5) {
				lastReportedPosition = currentTime;
				onProgress?.(currentTime);
			}
		}, 10000);
	});

	onDestroy(() => {
		if (hls) {
			hls.destroy();
		}
		if (controlsTimeout) {
			clearTimeout(controlsTimeout);
		}
		if (progressInterval) {
			clearInterval(progressInterval);
		}
		if (currentTime > 0) {
			onProgress?.(currentTime);
		}
	});

	function handleTimeUpdate() {
		if (videoElement) {
			currentTime = videoElement.currentTime;
			if (videoElement.buffered.length > 0) {
				buffered = videoElement.buffered.end(videoElement.buffered.length - 1);
			}
		}
	}

	function handleLoadedMetadata() {
		if (videoElement) {
			duration = videoElement.duration;
		}
	}

	function handlePlay() {
		isPlaying = true;
	}

	function handlePause() {
		isPlaying = false;
	}

	function handleEnded() {
		isPlaying = false;
		onEnded?.();
	}

	function handleVolumeChange() {
		if (videoElement) {
			volume = videoElement.volume;
			isMuted = videoElement.muted;
		}
	}

	function togglePlay() {
		if (!videoElement) return;
		if (isPlaying) {
			videoElement.pause();
		} else {
			videoElement.play();
		}
	}

	function toggleMute() {
		if (!videoElement) return;
		videoElement.muted = !videoElement.muted;
	}

	function setVolumeValue(newVolume: number) {
		if (!videoElement) return;
		videoElement.volume = Math.max(0, Math.min(1, newVolume));
	}

	function seek(time: number) {
		if (!videoElement) return;
		videoElement.currentTime = Math.max(0, Math.min(duration, time));
	}

	function seekRelative(delta: number) {
		seek(currentTime + delta);
	}

	function handleSeekInput(event: Event) {
		const input = event.target as HTMLInputElement;
		seek(parseFloat(input.value));
	}

	async function toggleFullscreen() {
		if (!containerElement) return;
		if (!document.fullscreenElement) {
			await containerElement.requestFullscreen();
			isFullscreen = true;
		} else {
			await document.exitFullscreen();
			isFullscreen = false;
		}
	}

	function handleMouseMove() {
		showControls = true;
		if (controlsTimeout) {
			clearTimeout(controlsTimeout);
		}
		controlsTimeout = setTimeout(() => {
			if (isPlaying) {
				showControls = false;
			}
		}, 3000);
	}

	function handleKeyDown(event: KeyboardEvent) {
		switch (event.key) {
			case ' ':
			case 'k':
				event.preventDefault();
				togglePlay();
				break;
			case 'ArrowLeft':
				event.preventDefault();
				seekRelative(-10);
				break;
			case 'ArrowRight':
				event.preventDefault();
				seekRelative(10);
				break;
			case 'ArrowUp':
				event.preventDefault();
				setVolumeValue(volume + 0.1);
				break;
			case 'ArrowDown':
				event.preventDefault();
				setVolumeValue(volume - 0.1);
				break;
			case 'm':
				event.preventDefault();
				toggleMute();
				break;
			case 'f':
				event.preventDefault();
				toggleFullscreen();
				break;
		}
	}
</script>

<div
	bind:this={containerElement}
	class="video-container relative aspect-video w-full bg-black"
	onmousemove={handleMouseMove}
	onmouseleave={() => isPlaying && (showControls = false)}
	role="toolbar"
	aria-label="Video player"
	tabindex="0"
	onkeydown={handleKeyDown}
>
	<!-- Video Element -->
	<video
		bind:this={videoElement}
		class="h-full w-full"
		{poster}
		ontimeupdate={handleTimeUpdate}
		onloadedmetadata={handleLoadedMetadata}
		onplay={handlePlay}
		onpause={handlePause}
		onended={handleEnded}
		onvolumechange={handleVolumeChange}
		onclick={togglePlay}
		playsinline
	>
		<track kind="captions" />
	</video>

	<!-- Click to play overlay -->
	{#if !isPlaying && currentTime === 0}
		<div
			class="absolute inset-0 flex cursor-pointer items-center justify-center"
			onclick={togglePlay}
			role="button"
			tabindex="0"
			onkeydown={(e) => e.key === 'Enter' && togglePlay()}
		>
			<div class="rounded-full bg-black/50 p-6">
				<Play class="h-16 w-16 text-white" />
			</div>
		</div>
	{/if}

	<!-- Controls -->
	<div
		class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent px-4 pb-4 pt-12 transition-opacity duration-300"
		class:opacity-0={!showControls}
		class:pointer-events-none={!showControls}
	>
		{#if title}
			<div class="mb-2 truncate text-sm text-white">{title}</div>
		{/if}

		<!-- Progress bar -->
		<div class="group relative mb-4 h-1 cursor-pointer rounded-full bg-white/30">
			<div class="absolute h-full rounded-full bg-white/50" style="width: {bufferedPercent}%"></div>
			<div class="absolute h-full rounded-full bg-primary" style="width: {progressPercent}%"></div>
			<input
				type="range"
				min="0"
				max={duration || 100}
				step="0.1"
				value={currentTime}
				oninput={handleSeekInput}
				class="absolute inset-0 h-full w-full cursor-pointer opacity-0"
			/>
			<div
				class="absolute -top-1 h-3 w-3 rounded-full bg-primary opacity-0 transition-opacity group-hover:opacity-100"
				style="left: calc({progressPercent}% - 6px)"
			></div>
		</div>

		<!-- Control buttons -->
		<div class="flex items-center gap-4">
			<button
				class="text-white transition-colors hover:text-primary"
				onclick={togglePlay}
				aria-label={isPlaying ? 'Pause' : 'Play'}
			>
				{#if isPlaying}
					<Pause class="h-6 w-6" />
				{:else}
					<Play class="h-6 w-6" />
				{/if}
			</button>

			<button
				class="text-white transition-colors hover:text-primary"
				onclick={() => seekRelative(-10)}
				aria-label="Skip back 10 seconds"
			>
				<SkipBack class="h-5 w-5" />
			</button>

			<button
				class="text-white transition-colors hover:text-primary"
				onclick={() => seekRelative(10)}
				aria-label="Skip forward 10 seconds"
			>
				<SkipForward class="h-5 w-5" />
			</button>

			<!-- Volume -->
			<div class="group flex items-center gap-2">
				<button
					class="text-white transition-colors hover:text-primary"
					onclick={toggleMute}
					aria-label={isMuted ? 'Unmute' : 'Mute'}
				>
					{#if isMuted || volume === 0}
						<VolumeX class="h-5 w-5" />
					{:else}
						<Volume2 class="h-5 w-5" />
					{/if}
				</button>
				<input
					type="range"
					min="0"
					max="1"
					step="0.05"
					value={isMuted ? 0 : volume}
					oninput={(e) => setVolumeValue(parseFloat((e.target as HTMLInputElement).value))}
					class="h-1 w-0 cursor-pointer appearance-none rounded-full bg-white/30 transition-all duration-200 group-hover:w-20 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white"
				/>
			</div>

			<!-- Time display -->
			<div class="ml-auto text-sm text-white">
				{formatTime(currentTime)} / {formatTime(duration)}
			</div>

			<!-- Fullscreen -->
			<button
				class="text-white transition-colors hover:text-primary"
				onclick={toggleFullscreen}
				aria-label={isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
			>
				{#if isFullscreen}
					<Minimize class="h-5 w-5" />
				{:else}
					<Maximize class="h-5 w-5" />
				{/if}
			</button>
		</div>
	</div>
</div>

<style>
	.video-container:fullscreen {
		width: 100vw;
		height: 100vh;
	}

	.video-container:fullscreen video {
		width: 100%;
		height: 100%;
		object-fit: contain;
	}
</style>
