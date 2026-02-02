<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { toast } from 'svelte-sonner';
  import * as api from '$lib/api';
  import type { Item, MediaFile, UserItemData, Person } from '$lib/types';
  import Button from '$lib/components/ui/button/button.svelte';
  import Badge from '$lib/components/ui/badge/badge.svelte';
  import {
    ArrowLeft,
    Heart,
    Check,
    Star,
    Clock,
    Calendar,
    Film,
    Tv,
    Music,
    Play,
    Loader2,
    AlertCircle,
    User,
    Users,
  } from 'lucide-svelte';

  const libraryId = $derived($page.params.libraryId!);
  const itemId = $derived($page.params.itemId!);

  let item = $state<Item | null>(null);
  let mediaFiles = $state<MediaFile[]>([]);
  let userItemData = $state<UserItemData | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let togglingFavorite = $state(false);
  let togglingPlayed = $state(false);

  // Determine if item is playable (has a universal version)
  const isPlayable = $derived(
    mediaFiles.some(f => f.serves_as_universal || f.role === 'universal')
  );

  // Icon based on item kind
  const ItemIcon = $derived.by(() => {
    if (!item) return Film;
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
        return Film;
    }
  });

  // Get poster image URL if available
  const posterImage = $derived.by(() => {
    if (!item?.images) return null;
    // Look for primary (poster) image first, then backdrop
    const poster = item.images.find(img => img.image_type === 'primary');
    if (poster) return poster.path;
    const backdrop = item.images.find(img => img.image_type === 'backdrop');
    return backdrop?.path ?? null;
  });

  // Get director(s) from people
  const directors = $derived.by(() => {
    if (!item?.people) return [];
    return item.people.filter(p => p.person_type === 'Director');
  });

  // Get cast members (actors), limited to top 10
  const cast = $derived.by(() => {
    if (!item?.people) return [];
    return item.people.filter(p => p.person_type === 'Actor').slice(0, 10);
  });

  // Get writers from people
  const writers = $derived.by(() => {
    if (!item?.people) return [];
    return item.people.filter(p => p.person_type === 'Writer');
  });

  // Check if we have any people data to display
  const hasPeopleData = $derived(directors.length > 0 || cast.length > 0 || writers.length > 0);

  onMount(async () => {
    await loadItemData();
  });

  async function loadItemData() {
    if (!itemId) return;
    loading = true;
    error = null;

    try {
      const [itemData, files, playbackInfo] = await Promise.all([
        api.getItem(itemId),
        api.getItemFiles(itemId).catch((e) => {
          toast.error('Failed to load media files');
          console.error('Failed to load media files:', e);
          return [];
        }),
        api.getPlaybackInfo(itemId).catch((e) => {
          toast.error('Failed to load playback info');
          console.error('Failed to load playback info:', e);
          return null;
        }),
      ]);

      item = itemData;
      mediaFiles = files;
      // Note: PlaybackInfo doesn't include user_data - it's updated via toggle functions
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load item';
    } finally {
      loading = false;
    }
  }

  async function handleToggleFavorite() {
    if (!item || togglingFavorite) return;
    togglingFavorite = true;

    try {
      userItemData = await api.toggleFavorite(item.id);
    } catch (e) {
      toast.error('Failed to update favorite status');
      console.error('Failed to toggle favorite:', e);
    } finally {
      togglingFavorite = false;
    }
  }

  async function handleTogglePlayed() {
    if (!item || togglingPlayed) return;
    togglingPlayed = true;

    try {
      if (userItemData?.played) {
        await api.markUnplayed(item.id);
        if (userItemData) {
          userItemData = { ...userItemData, played: false };
        }
      } else {
        await api.markPlayed(item.id);
        if (userItemData) {
          userItemData = { ...userItemData, played: true, play_count: userItemData.play_count + 1 };
        } else {
          // Create a minimal user data object if it doesn't exist
          userItemData = {
            item_id: item.id,
            user_id: '',
            played: true,
            play_count: 1,
            is_favorite: false,
            playback_position_ticks: null,
            last_played_date: null,
          };
        }
      }
    } catch (e) {
      toast.error('Failed to update played status');
      console.error('Failed to toggle played status:', e);
    } finally {
      togglingPlayed = false;
    }
  }

  function handlePlay() {
    if (!item || !isPlayable) return;
    goto(`/play/${item.id}`);
  }

  function handleBack() {
    goto(`/browse/${libraryId}`);
  }
</script>

<svelte:head>
  <title>{item?.name ?? 'Loading...'} - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <!-- Back button -->
  <Button variant="ghost" class="mb-4" onclick={handleBack}>
    <ArrowLeft class="w-4 h-4 mr-2" />
    Back to Library
  </Button>

  {#if loading}
    <div class="flex items-center justify-center py-20">
      <Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
    </div>
  {:else if error || !item}
    <div class="text-center py-20">
      <AlertCircle class="w-12 h-12 mx-auto text-destructive mb-4" />
      <p class="text-destructive text-lg">{error ?? 'Item not found'}</p>
      <Button variant="outline" class="mt-4" onclick={handleBack}>
        Return to Library
      </Button>
    </div>
  {:else}
    <div class="grid md:grid-cols-3 gap-8">
      <!-- Poster area -->
      <div class="md:col-span-1">
        <div class="aspect-[2/3] bg-muted rounded-lg flex items-center justify-center overflow-hidden shadow-lg">
          {#if posterImage}
            <img
              src={posterImage}
              alt={item.name}
              class="w-full h-full object-cover"
              loading="lazy"
            />
          {:else}
            <ItemIcon class="w-24 h-24 text-muted-foreground/30" />
          {/if}
        </div>

        <!-- Action buttons -->
        <div class="flex flex-col gap-3 mt-6">
          <!-- Play button -->
          {#if isPlayable}
            <Button
              variant="default"
              size="lg"
              class="w-full text-lg py-6"
              onclick={handlePlay}
            >
              <Play class="w-6 h-6 mr-2 fill-current" />
              Play
            </Button>
          {:else}
            <Button
              variant="secondary"
              size="lg"
              class="w-full text-lg py-6 cursor-not-allowed opacity-60"
              disabled
            >
              <AlertCircle class="w-6 h-6 mr-2" />
              Needs Conversion
            </Button>
            <p class="text-sm text-muted-foreground text-center">
              This item is not yet available for playback.
            </p>
          {/if}

          <!-- Favorite and Played toggles -->
          <div class="flex gap-2">
            <Button
              variant={userItemData?.is_favorite ? 'default' : 'outline'}
              class="flex-1"
              onclick={handleToggleFavorite}
              disabled={togglingFavorite}
            >
              {#if togglingFavorite}
                <Loader2 class="w-4 h-4 mr-2 animate-spin" />
              {:else}
                <Heart class="w-4 h-4 mr-2 {userItemData?.is_favorite ? 'fill-current' : ''}" />
              {/if}
              Favorite
            </Button>

            <Button
              variant={userItemData?.played ? 'default' : 'outline'}
              class="flex-1"
              onclick={handleTogglePlayed}
              disabled={togglingPlayed}
            >
              {#if togglingPlayed}
                <Loader2 class="w-4 h-4 mr-2 animate-spin" />
              {:else}
                <Check class="w-4 h-4 mr-2" />
              {/if}
              {userItemData?.played ? 'Played' : 'Mark Played'}
            </Button>
          </div>
        </div>
      </div>

      <!-- Details section -->
      <div class="md:col-span-2">
        <!-- Title -->
        <div class="mb-4">
          <h1 class="text-3xl font-bold">{item.name}</h1>

          {#if item.original_title && item.original_title !== item.name}
            <p class="text-lg text-muted-foreground mt-1">{item.original_title}</p>
          {/if}

          {#if item.tagline}
            <p class="text-muted-foreground italic mt-2">{item.tagline}</p>
          {/if}
        </div>

        <!-- Metadata badges -->
        <div class="flex flex-wrap gap-2 mb-6">
          {#if item.production_year}
            <Badge variant="secondary">
              <Calendar class="w-3 h-3 mr-1" />
              {item.production_year}
            </Badge>
          {/if}

          {#if item.runtime_ticks}
            <Badge variant="secondary">
              <Clock class="w-3 h-3 mr-1" />
              {api.formatRuntime(item.runtime_ticks)}
            </Badge>
          {/if}

          {#if item.community_rating}
            <Badge variant="secondary">
              <Star class="w-3 h-3 mr-1 fill-yellow-500 text-yellow-500" />
              {item.community_rating.toFixed(1)}
            </Badge>
          {/if}

          {#if item.official_rating}
            <Badge variant="outline">{item.official_rating}</Badge>
          {/if}

          {#if item.hdr_type}
            <Badge variant="default">{item.hdr_type}</Badge>
          {/if}

          {#if item.dolby_vision_profile}
            <Badge variant="default">Dolby Vision</Badge>
          {/if}
        </div>

        <!-- Overview -->
        {#if item.overview}
          <div class="mb-6">
            <h2 class="text-lg font-semibold mb-2">Overview</h2>
            <p class="text-muted-foreground leading-relaxed">{item.overview}</p>
          </div>
        {/if}

        <!-- Genres -->
        {#if item.genres.length > 0}
          <div class="mb-6">
            <h2 class="text-lg font-semibold mb-2">Genres</h2>
            <div class="flex flex-wrap gap-2">
              {#each item.genres as genre (genre)}
                <Badge variant="outline">{genre}</Badge>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Cast & Crew -->
        {#if hasPeopleData}
          <div class="mb-6">
            <h2 class="text-lg font-semibold mb-3">Cast & Crew</h2>

            <!-- Director(s) -->
            {#if directors.length > 0}
              <div class="mb-4">
                <h3 class="text-sm font-medium text-muted-foreground mb-2">
                  {directors.length === 1 ? 'Director' : 'Directors'}
                </h3>
                <div class="flex flex-wrap gap-2">
                  {#each directors as director (director.name)}
                    <Badge variant="secondary" class="flex items-center gap-1">
                      <User class="w-3 h-3" />
                      {director.name}
                    </Badge>
                  {/each}
                </div>
              </div>
            {/if}

            <!-- Writer(s) -->
            {#if writers.length > 0}
              <div class="mb-4">
                <h3 class="text-sm font-medium text-muted-foreground mb-2">
                  {writers.length === 1 ? 'Writer' : 'Writers'}
                </h3>
                <div class="flex flex-wrap gap-2">
                  {#each writers.slice(0, 5) as writer (writer.name)}
                    <Badge variant="secondary" class="flex items-center gap-1">
                      {writer.name}
                    </Badge>
                  {/each}
                  {#if writers.length > 5}
                    <Badge variant="outline">+{writers.length - 5} more</Badge>
                  {/if}
                </div>
              </div>
            {/if}

            <!-- Cast -->
            {#if cast.length > 0}
              <div>
                <h3 class="text-sm font-medium text-muted-foreground mb-2">Cast</h3>
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
                  {#each cast as actor (actor.name)}
                    <div class="flex items-center gap-3 p-2 rounded-lg bg-muted/50">
                      {#if actor.image_url}
                        <img
                          src={actor.image_url}
                          alt={actor.name}
                          class="w-10 h-10 rounded-full object-cover"
                          loading="lazy"
                        />
                      {:else}
                        <div class="w-10 h-10 rounded-full bg-muted flex items-center justify-center">
                          <User class="w-5 h-5 text-muted-foreground" />
                        </div>
                      {/if}
                      <div class="flex-1 min-w-0">
                        <p class="font-medium text-sm truncate">{actor.name}</p>
                        {#if actor.role}
                          <p class="text-xs text-muted-foreground truncate">as {actor.role}</p>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        {/if}

        <!-- Studios -->
        {#if item.studios.length > 0}
          <div class="mb-6">
            <h2 class="text-lg font-semibold mb-2">Studios</h2>
            <p class="text-muted-foreground">{item.studios.join(', ')}</p>
          </div>
        {/if}

        <!-- Episode info for TV -->
        {#if item.item_kind === 'episode'}
          <div class="mb-6">
            <h2 class="text-lg font-semibold mb-2">Episode Info</h2>
            <div class="text-muted-foreground">
              {#if item.parent_index_number !== null}
                <span>Season {item.parent_index_number}</span>
              {/if}
              {#if item.parent_index_number !== null && item.index_number !== null}
                <span> - </span>
              {/if}
              {#if item.index_number !== null}
                <span>Episode {item.index_number}</span>
              {/if}
            </div>
          </div>
        {/if}

        <!-- Technical info (minimal for user-facing) -->
        {#if item.resolution || item.video_codec}
          <div class="mb-6">
            <h2 class="text-lg font-semibold mb-2">Quality</h2>
            <div class="flex flex-wrap gap-2">
              {#if item.resolution}
                <Badge variant="secondary">{item.resolution}</Badge>
              {/if}
              {#if item.video_codec}
                <Badge variant="secondary" class="uppercase">{item.video_codec}</Badge>
              {/if}
              {#if item.audio_codec}
                <Badge variant="secondary" class="uppercase">{item.audio_codec}</Badge>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>
