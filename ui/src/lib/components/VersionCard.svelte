<script lang="ts">
  import type { MediaFile } from '$lib/types';
  import { formatBytes } from '$lib/api';
  import { cn } from '$lib/utils';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import {
    HardDrive,
    Play,
    Monitor,
    Loader2,
  } from 'lucide-svelte';

  interface Props {
    mediaFile: MediaFile;
    hasUniversal?: boolean;
    converting?: boolean;
    onConvert?: () => void;
  }

  let { mediaFile, hasUniversal = false, converting = false, onConvert }: Props = $props();

  // Determine role label
  const roleLabel = $derived(mediaFile.role === 'source' ? 'Source' : mediaFile.role === 'universal' ? 'Universal' : mediaFile.role);

  // Format resolution
  const resolution = $derived.by(() => {
    if (!mediaFile.width || !mediaFile.height) return null;
    if (mediaFile.height >= 2160) return '4K';
    if (mediaFile.height >= 1080) return '1080p';
    if (mediaFile.height >= 720) return '720p';
    return `${mediaFile.height}p`;
  });

  // Format resolution full
  const resolutionFull = $derived.by(() => {
    if (!mediaFile.width || !mediaFile.height) return null;
    return `${mediaFile.width}x${mediaFile.height}`;
  });

  // Show "Create Universal Copy" button
  const showConvertButton = $derived(
    mediaFile.role === 'source' &&
    !hasUniversal &&
    mediaFile.can_be_profile_b &&
    onConvert
  );
</script>

<div class="border rounded-lg p-4 bg-card">
  <!-- Header with role and size -->
  <div class="flex items-center justify-between mb-3">
    <div class="flex items-center gap-2">
      <Badge variant={mediaFile.serves_as_universal ? 'secondary' : 'default'}>
        {roleLabel}
      </Badge>
      {#if mediaFile.serves_as_universal}
        <Badge variant="outline" class="gap-1">
          <Monitor class="w-3 h-3" />
          Web playable
        </Badge>
      {/if}
      {#if mediaFile.is_hdr}
        <Badge variant="secondary">HDR</Badge>
      {/if}
    </div>
    <span class="text-sm text-muted-foreground font-medium">
      {formatBytes(mediaFile.file_size)}
    </span>
  </div>

  <!-- Specs grid -->
  <div class="grid grid-cols-2 sm:grid-cols-4 gap-3 text-sm mb-3">
    {#if resolution}
      <div>
        <span class="text-muted-foreground">Resolution:</span>
        <span class="ml-1 font-medium">{resolution}</span>
        {#if resolutionFull}
          <span class="text-muted-foreground text-xs ml-1">({resolutionFull})</span>
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
  <div class="flex items-center gap-2 text-xs text-muted-foreground/70 truncate mb-3">
    <HardDrive class="w-3 h-3 flex-shrink-0" />
    <span class="truncate" title={mediaFile.file_path}>{mediaFile.file_path}</span>
  </div>

  <!-- Action area -->
  {#if showConvertButton}
    <div class="pt-3 border-t">
      <Button
        variant="default"
        size="sm"
        disabled={converting}
        onclick={onConvert}
      >
        {#if converting}
          <Loader2 class="w-4 h-4 mr-2 animate-spin" />
        {:else}
          <Play class="w-4 h-4 mr-2" />
        {/if}
        Create Universal Copy
      </Button>
    </div>
  {/if}
</div>
