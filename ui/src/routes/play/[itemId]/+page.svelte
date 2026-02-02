<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import * as api from '$lib/api';
  import type { Item, PlaybackInfo, MediaSourceInfo } from '$lib/types';
  import VideoPlayer from '$lib/components/VideoPlayer.svelte';
  import Button from '$lib/components/ui/button/button.svelte';
  import { ArrowLeft, Loader2, AlertCircle, ExternalLink } from 'lucide-svelte';

  const itemId = $derived($page.params.itemId!);
  const startFromBeginning = $derived($page.url.searchParams.get('start') === '0');

  let item = $state<Item | null>(null);
  let playbackInfo = $state<PlaybackInfo | null>(null);
  let selectedSource = $state<MediaSourceInfo | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let streamUrl = $state<string | null>(null);
  let startPosition = $state(0);

  onMount(async () => {
    await loadPlaybackInfo();
  });

  // Pick the best media source - all sources from API are now web-playable (Profile B)
  function selectBestSource(sources: MediaSourceInfo[]): MediaSourceInfo | null {
    if (sources.length === 0) return null;
    // Just use the first source - backend only returns playable sources
    return sources[0];
  }

  async function loadPlaybackInfo() {
    if (!itemId) return;
    loading = true;
    error = null;

    try {
      [item, playbackInfo] = await Promise.all([
        api.getItem(itemId),
        api.getPlaybackInfo(itemId),
      ]);

      if (!playbackInfo || !playbackInfo.media_sources || playbackInfo.media_sources.length === 0) {
        error = 'No playback sources available';
        return;
      }

      // Select the best source
      selectedSource = selectBestSource(playbackInfo.media_sources);

      if (!selectedSource) {
        error = 'No playable source found';
        return;
      }

      // Determine stream URL - prefer HLS over direct
      if (selectedSource.hls_url) {
        streamUrl = selectedSource.hls_url;
      } else if (selectedSource.direct_stream_url) {
        streamUrl = selectedSource.direct_stream_url;
      } else {
        error = 'No stream URL available';
        return;
      }

      // TODO: Get user data for start position when user system is implemented
      // For now, always start from beginning
      startPosition = 0;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load playback info';
    } finally {
      loading = false;
    }
  }

  async function handleProgress(positionTicks: number) {
    try {
      await api.updatePlaybackPosition(itemId, positionTicks);
    } catch (e) {
      console.error('Failed to update playback position:', e);
    }
  }

  async function handleEnded() {
    try {
      await api.markPlayed(itemId);
    } catch (e) {
      console.error('Failed to mark as played:', e);
    }
    // Optionally return to item detail
    // if (item) goto(`/browse/${item.library_id}/${itemId}`);
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
  <title>{item?.name ?? 'Playing'} - Sceneforged</title>
</svelte:head>

<div class="min-h-screen bg-black flex flex-col">
  <!-- Header -->
  <div class="absolute top-0 left-0 right-0 z-10 p-4 bg-gradient-to-b from-black/80 to-transparent">
    <div class="flex items-center gap-4">
      <Button variant="ghost" size="icon" class="text-white hover:bg-white/20" onclick={goBack}>
        <ArrowLeft class="w-5 h-5" />
      </Button>
      {#if item}
        <div class="text-white">
          <h1 class="font-medium">{item.name}</h1>
          {#if item.production_year}
            <p class="text-sm text-white/70">{item.production_year}</p>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  <!-- Main content -->
  <div class="flex-1 flex items-center justify-center">
    {#if loading}
      <div class="flex flex-col items-center gap-4 text-white">
        <Loader2 class="w-12 h-12 animate-spin" />
        <p>Loading...</p>
      </div>
    {:else if error}
      <div class="flex flex-col items-center gap-4 text-white text-center max-w-md px-4">
        <AlertCircle class="w-12 h-12 text-destructive" />
        <h2 class="text-xl font-medium">Playback Error</h2>
        <p class="text-white/70">{error}</p>
        <div class="flex gap-4 mt-4">
          <Button variant="outline" onclick={goBack}>
            <ArrowLeft class="w-4 h-4 mr-2" />
            Go Back
          </Button>
          <Button onclick={loadPlaybackInfo}>
            Try Again
          </Button>
        </div>

        {#if selectedSource?.direct_stream_url}
          <a
            href={selectedSource.direct_stream_url}
            target="_blank"
            rel="noopener noreferrer"
            class="text-sm text-white/50 hover:text-white flex items-center gap-1 mt-4"
          >
            Open direct stream
            <ExternalLink class="w-3 h-3" />
          </a>
        {/if}
      </div>
    {:else if streamUrl}
      <div class="w-full h-full max-w-screen-2xl">
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
