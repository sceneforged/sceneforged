<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import Hls from 'hls.js';
  import { Play, Pause, Volume2, VolumeX, Maximize, Minimize, SkipBack, SkipForward } from 'lucide-svelte';
  import { formatTimestamp } from '$lib/api';

  interface Props {
    src: string;
    poster?: string;
    title?: string;
    startPosition?: number; // in ticks (100-nanosecond intervals)
    onProgress?: (positionTicks: number) => void;
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
    onError,
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

  // Convert ticks to seconds
  const ticksToSeconds = (ticks: number) => ticks / 10_000_000;
  const secondsToTicks = (seconds: number) => Math.floor(seconds * 10_000_000);

  // Progress reporting interval (every 10 seconds)
  let progressInterval: ReturnType<typeof setInterval> | null = null;
  let lastReportedPosition = 0;

  onMount(() => {
    if (!videoElement) return;

    // Check if HLS is supported
    if (Hls.isSupported()) {
      hls = new Hls({
        enableWorker: true,
        lowLatencyMode: false,
        backBufferLength: 90,
      });

      hls.loadSource(src);
      hls.attachMedia(videoElement);

      hls.on(Hls.Events.MANIFEST_PARSED, () => {
        // Set start position if provided
        if (startPosition > 0) {
          videoElement.currentTime = ticksToSeconds(startPosition);
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
      // Safari native HLS support
      videoElement.src = src;
      if (startPosition > 0) {
        videoElement.currentTime = ticksToSeconds(startPosition);
      }
    } else {
      onError?.('HLS is not supported in this browser');
    }

    // Start progress reporting
    progressInterval = setInterval(() => {
      if (isPlaying && videoElement && Math.abs(currentTime - lastReportedPosition) > 5) {
        lastReportedPosition = currentTime;
        onProgress?.(secondsToTicks(currentTime));
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
    // Report final position on unmount
    if (currentTime > 0) {
      onProgress?.(secondsToTicks(currentTime));
    }
  });

  function handleTimeUpdate() {
    if (videoElement) {
      currentTime = videoElement.currentTime;

      // Update buffered amount
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

  function setVolume(newVolume: number) {
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
        setVolume(volume + 0.1);
        break;
      case 'ArrowDown':
        event.preventDefault();
        setVolume(volume - 0.1);
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

  // Format progress percentage
  const progressPercent = $derived(duration > 0 ? (currentTime / duration) * 100 : 0);
  const bufferedPercent = $derived(duration > 0 ? (buffered / duration) * 100 : 0);
</script>

<div
  bind:this={containerElement}
  class="video-container relative bg-black w-full aspect-video"
  onmousemove={handleMouseMove}
  onmouseleave={() => isPlaying && (showControls = false)}
  role="application"
  aria-label="Video player"
  tabindex="0"
  onkeydown={handleKeyDown}
>
  <!-- Video Element -->
  <video
    bind:this={videoElement}
    class="w-full h-full"
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
      class="absolute inset-0 flex items-center justify-center cursor-pointer"
      onclick={togglePlay}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === 'Enter' && togglePlay()}
    >
      <div class="bg-black/50 rounded-full p-6">
        <Play class="w-16 h-16 text-white" />
      </div>
    </div>
  {/if}

  <!-- Controls -->
  <div
    class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent px-4 pb-4 pt-12 transition-opacity duration-300"
    class:opacity-0={!showControls}
    class:pointer-events-none={!showControls}
  >
    <!-- Title -->
    {#if title}
      <div class="text-white text-sm mb-2 truncate">{title}</div>
    {/if}

    <!-- Progress bar -->
    <div class="relative h-1 bg-white/30 rounded-full mb-4 group cursor-pointer">
      <!-- Buffered -->
      <div
        class="absolute h-full bg-white/50 rounded-full"
        style="width: {bufferedPercent}%"
      ></div>
      <!-- Progress -->
      <div
        class="absolute h-full bg-primary rounded-full"
        style="width: {progressPercent}%"
      ></div>
      <!-- Seek input -->
      <input
        type="range"
        min="0"
        max={duration || 100}
        step="0.1"
        value={currentTime}
        oninput={handleSeekInput}
        class="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
      />
      <!-- Hover indicator -->
      <div
        class="absolute h-3 w-3 bg-primary rounded-full -top-1 opacity-0 group-hover:opacity-100 transition-opacity"
        style="left: calc({progressPercent}% - 6px)"
      ></div>
    </div>

    <!-- Control buttons -->
    <div class="flex items-center gap-4">
      <!-- Play/Pause -->
      <button
        class="text-white hover:text-primary transition-colors"
        onclick={togglePlay}
        aria-label={isPlaying ? 'Pause' : 'Play'}
      >
        {#if isPlaying}
          <Pause class="w-6 h-6" />
        {:else}
          <Play class="w-6 h-6" />
        {/if}
      </button>

      <!-- Skip backward -->
      <button
        class="text-white hover:text-primary transition-colors"
        onclick={() => seekRelative(-10)}
        aria-label="Skip back 10 seconds"
      >
        <SkipBack class="w-5 h-5" />
      </button>

      <!-- Skip forward -->
      <button
        class="text-white hover:text-primary transition-colors"
        onclick={() => seekRelative(10)}
        aria-label="Skip forward 10 seconds"
      >
        <SkipForward class="w-5 h-5" />
      </button>

      <!-- Volume -->
      <div class="flex items-center gap-2 group">
        <button
          class="text-white hover:text-primary transition-colors"
          onclick={toggleMute}
          aria-label={isMuted ? 'Unmute' : 'Mute'}
        >
          {#if isMuted || volume === 0}
            <VolumeX class="w-5 h-5" />
          {:else}
            <Volume2 class="w-5 h-5" />
          {/if}
        </button>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          value={isMuted ? 0 : volume}
          oninput={(e) => setVolume(parseFloat((e.target as HTMLInputElement).value))}
          class="w-0 group-hover:w-20 transition-all duration-200 h-1 bg-white/30 rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:rounded-full"
        />
      </div>

      <!-- Time display -->
      <div class="text-white text-sm ml-auto">
        {formatTimestamp(secondsToTicks(currentTime))} / {formatTimestamp(secondsToTicks(duration))}
      </div>

      <!-- Fullscreen -->
      <button
        class="text-white hover:text-primary transition-colors"
        onclick={toggleFullscreen}
        aria-label={isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
      >
        {#if isFullscreen}
          <Minimize class="w-5 h-5" />
        {:else}
          <Maximize class="w-5 h-5" />
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
