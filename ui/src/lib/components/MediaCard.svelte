<script lang="ts">
  import type { Item, MediaFile } from '$lib/types';
  import { formatRuntime, formatBytes } from '$lib/api';
  import { Film, Tv, Music, FolderOpen, Star, Play, Check, Clock } from 'lucide-svelte';
  import Badge from './ui/badge/badge.svelte';

  interface Props {
    item: Item;
    onclick?: () => void;
    playbackPosition?: number | null;
    played?: boolean;
    mediaFiles?: MediaFile[];
  }

  let { item, onclick, playbackPosition, played, mediaFiles }: Props = $props();

  function getItemProfiles(files: MediaFile[] | undefined): string {
    if (!files || files.length === 0) return '';
    const hasA = files.some(f => !f.serves_as_universal);
    const hasB = files.some(f => f.serves_as_universal);
    if (hasA && hasB) return 'A+B';
    if (hasB) return 'B';
    if (hasA) return 'A';
    return '';
  }

  const profiles = $derived(getItemProfiles(mediaFiles));

  // Get appropriate icon based on item kind
  const Icon = $derived.by(() => {
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

  // Calculate progress percentage
  const progressPercent = $derived(
    playbackPosition && item.runtime_ticks
      ? Math.min(100, (playbackPosition / item.runtime_ticks) * 100)
      : 0
  );
</script>

<button
  class="group relative flex flex-col bg-card rounded-lg overflow-hidden shadow-sm hover:shadow-md transition-shadow cursor-pointer text-left w-full"
  {onclick}
>
  <!-- Poster/Thumbnail area -->
  <div class="relative aspect-[2/3] bg-muted flex items-center justify-center overflow-hidden">
    <!-- Placeholder icon -->
    <Icon class="w-16 h-16 text-muted-foreground/30" />

    <!-- Play overlay on hover -->
    <div class="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
      <div class="bg-primary rounded-full p-3">
        <Play class="w-8 h-8 text-primary-foreground" />
      </div>
    </div>

    <!-- Played indicator -->
    {#if played}
      <div class="absolute top-2 right-2 bg-green-600 rounded-full p-1">
        <Check class="w-4 h-4 text-white" />
      </div>
    {/if}

    <!-- HDR/DV badges -->
    <div class="absolute bottom-2 left-2 flex gap-1">
      {#if item.hdr_type}
        <Badge variant="secondary" class="text-xs px-1.5 py-0.5">
          {item.hdr_type}
        </Badge>
      {/if}
      {#if item.dolby_vision_profile}
        <Badge variant="secondary" class="text-xs px-1.5 py-0.5">
          DV
        </Badge>
      {/if}
      {#if profiles}
        <Badge variant="default" class="text-xs px-1.5 py-0.5 bg-blue-600">
          {profiles}
        </Badge>
      {/if}
    </div>

    <!-- Progress bar -->
    {#if progressPercent > 0 && !played}
      <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
        <div
          class="h-full bg-primary transition-all"
          style="width: {progressPercent}%"
        ></div>
      </div>
    {/if}
  </div>

  <!-- Content -->
  <div class="p-3 flex-1 flex flex-col gap-1">
    <h3 class="font-medium text-sm line-clamp-2 text-foreground">{item.name}</h3>

    <div class="flex items-center gap-2 text-xs text-muted-foreground mt-auto">
      <!-- Year -->
      {#if item.production_year}
        <span>{item.production_year}</span>
      {/if}

      <!-- Runtime -->
      {#if item.runtime_ticks}
        <span class="flex items-center gap-1">
          <Clock class="w-3 h-3" />
          {formatRuntime(item.runtime_ticks)}
        </span>
      {/if}

      <!-- Rating -->
      {#if item.community_rating}
        <span class="flex items-center gap-1">
          <Star class="w-3 h-3 fill-yellow-500 text-yellow-500" />
          {item.community_rating.toFixed(1)}
        </span>
      {/if}
    </div>

    <!-- Resolution/Codec info -->
    {#if item.resolution || item.video_codec}
      <div class="flex items-center gap-1 text-xs text-muted-foreground">
        {#if item.resolution}
          <span>{item.resolution}</span>
        {/if}
        {#if item.video_codec}
          <span class="uppercase">{item.video_codec}</span>
        {/if}
      </div>
    {/if}

    <!-- Episode info -->
    {#if item.item_kind === 'episode' && (item.parent_index_number !== null || item.index_number !== null)}
      <div class="text-xs text-muted-foreground">
        {#if item.parent_index_number !== null}
          S{item.parent_index_number.toString().padStart(2, '0')}
        {/if}
        {#if item.index_number !== null}
          E{item.index_number.toString().padStart(2, '0')}
        {/if}
      </div>
    {/if}
  </div>
</button>
