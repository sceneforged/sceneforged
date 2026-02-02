<script lang="ts">
  import type { Item, Profile } from '$lib/types';
  import { formatRuntime, formatBytes } from '$lib/api';
  import { Film, Tv, Music, FolderOpen, Star, Play, Check, Clock } from 'lucide-svelte';
  import Badge from './ui/badge/badge.svelte';
  import ProfileBadge from './ProfileBadge.svelte';
  import { goto } from '$app/navigation';

  interface Props {
    item: Item;
    onclick?: () => void;
    playbackPosition?: number | null;
    played?: boolean;
    libraryId?: string;
    adminMode?: boolean;
  }

  let { item, onclick, playbackPosition, played, libraryId, adminMode = false }: Props = $props();

  // Determine if item is web-playable using profile_b flag
  const isWebPlayable = $derived(item.has_profile_b);

  // Resolve library ID from prop or item
  const resolvedLibraryId = $derived(libraryId ?? item.library_id);

  // Handle poster click - navigate to play page (only if web-playable) or admin/detail page
  function handlePosterClick(e: MouseEvent) {
    e.stopPropagation();
    if (adminMode) {
      goto(`/admin/item/${item.id}`);
    } else if (isWebPlayable) {
      goto(`/play/${item.id}`);
    } else {
      // Navigate to detail page instead when no web-playable version
      goto(`/browse/${resolvedLibraryId}/${item.id}`);
    }
  }

  // Handle title click - navigate to browse/details page or admin page
  function handleTitleClick(e: MouseEvent) {
    e.stopPropagation();
    if (adminMode) {
      goto(`/admin/item/${item.id}`);
    } else {
      goto(`/browse/${resolvedLibraryId}/${item.id}`);
    }
  }

  // Derive profile badge from item flags
  const profile = $derived.by(() => {
    if (item.has_profile_a && item.has_profile_b) return 'AB';
    if (item.has_profile_b) return 'B';
    if (item.has_profile_a) return 'A';
    if (item.has_profile_c) return 'C';
    return null;
  });

  // Determine resolution tier from item resolution string or compute from width
  const resolutionTier = $derived.by(() => {
    const res = item.resolution?.toLowerCase();
    if (res) {
      if (res.includes('4k') || res.includes('2160') || res.includes('uhd')) return 'UHD';
      if (res.includes('1080') || res.includes('fhd')) return 'FHD';
      if (res.includes('720') || res.includes('hd')) return 'HD';
      return null;
    }
    return null;
  });

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

<div
  class="group relative flex flex-col bg-card rounded-lg overflow-hidden shadow-sm hover:shadow-md transition-shadow text-left w-full"
>
  <!-- Poster/Thumbnail area -->
  <button
    type="button"
    class="relative aspect-[2/3] bg-muted flex items-center justify-center overflow-hidden w-full cursor-pointer"
    onclick={handlePosterClick}
  >
    <!-- Placeholder icon -->
    <Icon class="w-16 h-16 text-muted-foreground/30" />

    <!-- Play overlay on hover (only show if web-playable) -->
    {#if isWebPlayable}
      <div class="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
        <div class="bg-primary rounded-full p-3">
          <Play class="w-8 h-8 text-primary-foreground" />
        </div>
      </div>
    {/if}

    <!-- No Web Version overlay (shown when not web-playable) -->
    {#if !isWebPlayable}
      <div class="absolute inset-0 bg-black/60 flex items-center justify-center">
        <span class="text-white text-sm font-medium px-2 py-1 text-center">No Web Version</span>
      </div>
    {/if}

    <!-- Resolution and HDR/DV badges -->
    <div class="absolute bottom-2 left-2 flex gap-1">
      {#if resolutionTier}
        <Badge variant="secondary" class="text-xs px-1.5 py-0.5 {resolutionTier === 'UHD' ? 'bg-purple-600 text-white' : ''}">
          {resolutionTier}
        </Badge>
      {/if}
      {#if item.hdr_type}
        <Badge variant="secondary" class="text-xs px-1.5 py-0.5 bg-amber-600 text-white">
          {item.hdr_type}
        </Badge>
      {/if}
      {#if item.dolby_vision_profile}
        <Badge variant="secondary" class="text-xs px-1.5 py-0.5 bg-black text-white">
          DV
        </Badge>
      {/if}
    </div>

    <!-- Profile badge and played indicator in top-right corner -->
    <div class="absolute top-2 right-2 flex items-center gap-1">
      {#if profile}
        <ProfileBadge {profile} />
      {/if}
      {#if played}
        <div class="bg-green-600 rounded-full p-1">
          <Check class="w-4 h-4 text-white" />
        </div>
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
  </button>

  <!-- Content -->
  <div class="p-3 flex-1 flex flex-col gap-1">
    <button
      type="button"
      class="font-medium text-sm line-clamp-2 text-foreground hover:text-primary transition-colors cursor-pointer text-left"
      onclick={handleTitleClick}
    >
      {item.name}
    </button>

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

    <!-- Codec info (only show if no resolution tier badge displayed) -->
    {#if !resolutionTier && item.video_codec}
      <div class="flex items-center gap-1 text-xs text-muted-foreground">
        <span class="uppercase">{item.video_codec}</span>
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
</div>
