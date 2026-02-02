<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { toast } from 'svelte-sonner';
  import * as api from '$lib/api';
  import type { Item, MediaFile, UserItemData } from '$lib/types';
  import Button from '$lib/components/ui/button/button.svelte';
  import Badge from '$lib/components/ui/badge/badge.svelte';
  import ProfileBadge from '$lib/components/ProfileBadge.svelte';
  import {
    ArrowLeft,
    Heart,
    Check,
    Star,
    Clock,
    Calendar,
    Film,
    Tv,
    HardDrive,
    Loader2,
    AlertTriangle,
  } from 'lucide-svelte';

  const itemId = $derived($page.params.itemId!);

  let item = $state<Item | null>(null);
  let mediaFiles = $state<MediaFile[]>([]);
  let userItemData = $state<UserItemData | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let conversionOptions = $state<api.ConversionOptionsResponse | null>(null);
  let loadingConversion = $state(false);
  let converting = $state(false);

  onMount(async () => {
    await loadItem();
    await loadConversionOptions();
  });

  async function loadItem() {
    if (!itemId) return;
    loading = true;
    error = null;

    try {
      const [itemData, files] = await Promise.all([
        api.getItem(itemId),
        api.getItemFiles(itemId).catch(() => []),
      ]);
      item = itemData;
      mediaFiles = files;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load item';
    } finally {
      loading = false;
    }
  }

  async function loadConversionOptions() {
    if (!itemId) return;
    loadingConversion = true;

    try {
      conversionOptions = await api.getConversionOptions(itemId);
    } catch (e) {
      console.error('Failed to load conversion options:', e);
      conversionOptions = null;
    } finally {
      loadingConversion = false;
    }
  }

  async function handleToggleFavorite() {
    if (!item) return;
    try {
      userItemData = await api.toggleFavorite(item.id);
    } catch (e) {
      console.error('Failed to toggle favorite:', e);
    }
  }

  async function handleMarkPlayed() {
    if (!item) return;
    try {
      await api.markPlayed(item.id);
      if (userItemData) {
        userItemData = { ...userItemData, played: true, play_count: userItemData.play_count + 1 };
      }
    } catch (e) {
      console.error('Failed to mark played:', e);
    }
  }

  async function handleMarkUnplayed() {
    if (!item) return;
    try {
      await api.markUnplayed(item.id);
      if (userItemData) {
        userItemData = { ...userItemData, played: false };
      }
    } catch (e) {
      console.error('Failed to mark unplayed:', e);
    }
  }

  async function handleConvert(targetProfiles: string[]) {
    if (!item || converting) return;
    converting = true;

    try {
      const response = await api.convertItem(item.id, targetProfiles);
      toast.success(`Conversion started (${response.job_ids.length} job${response.job_ids.length > 1 ? 's' : ''} created)`);

      // Reload conversion options after starting conversion
      await loadConversionOptions();
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to start conversion';
      toast.error(message);
    } finally {
      converting = false;
    }
  }

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
      default:
        return Film;
    }
  });

  // Determine current profile from media files
  const currentProfile = $derived.by(() => {
    if (!mediaFiles.length) return null;

    const hasA = mediaFiles.some(f => f.profile === 'A');
    const hasB = mediaFiles.some(f => f.profile === 'B');

    if (hasA && hasB) return 'AB';
    if (hasA) return 'A';
    if (hasB) return 'B';
    return 'C';
  });

  // Check if item needs conversion
  const needsConversion = $derived.by(() => {
    if (!conversionOptions) return false;
    return conversionOptions.viable_targets.length > 0;
  });

  // Generate conversion button options
  const conversionActions = $derived.by(() => {
    if (!conversionOptions) return [];

    const actions: { label: string; profiles: string[] }[] = [];
    const viable = conversionOptions.viable_targets;
    const current = conversionOptions.current_profiles;

    // Check if we can create both A and B
    const canCreateA = viable.includes('A') && !current.includes('A');
    const canCreateB = viable.includes('B') && !current.includes('B');

    if (canCreateA && canCreateB) {
      actions.push({ label: 'Convert to A+B', profiles: ['A', 'B'] });
    }

    if (canCreateB && !canCreateA) {
      actions.push({ label: 'Convert to B only', profiles: ['B'] });
    }

    if (canCreateA && !canCreateB) {
      actions.push({ label: 'Convert to A only', profiles: ['A'] });
    }

    return actions;
  });

  // Generate source analysis text
  const sourceAnalysis = $derived.by(() => {
    if (!mediaFiles.length || !conversionOptions) return null;

    const sourceFile = mediaFiles.find(f => f.role === 'source') || mediaFiles[0];
    const parts: string[] = [];

    if (sourceFile.width && sourceFile.height) {
      const resolution = sourceFile.height >= 2160 ? '4K' : sourceFile.height >= 1080 ? '1080p' : `${sourceFile.height}p`;
      parts.push(resolution);
    }

    if (sourceFile.is_hdr) {
      parts.push('DV HDR10');
    }

    if (sourceFile.video_codec) {
      parts.push(sourceFile.video_codec.toUpperCase());
    }

    const viable = conversionOptions.viable_targets;
    const qualifies: string[] = [];

    if (viable.includes('A')) {
      qualifies.push('qualifies for Profile A');
    }

    if (viable.includes('B')) {
      qualifies.push('can generate Profile B (universal)');
    }

    return {
      specs: parts.join(' '),
      capabilities: qualifies,
    };
  });

</script>

<svelte:head>
  <title>{item?.name ?? 'Loading...'} - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <!-- Back button -->
  <Button variant="ghost" class="mb-4" onclick={() => goto('/library')}>
    <ArrowLeft class="w-4 h-4 mr-2" />
    Back to Library
  </Button>

  {#if loading}
    <div class="flex items-center justify-center py-20">
      <Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
    </div>
  {:else if error || !item}
    <div class="text-center py-20">
      <p class="text-destructive">{error ?? 'Item not found'}</p>
      <Button variant="outline" class="mt-4" onclick={() => goto('/library')}>
        Return to Library
      </Button>
    </div>
  {:else}
    <div class="grid md:grid-cols-3 gap-8">
      <!-- Poster/Thumbnail -->
      <div class="md:col-span-1">
        <div class="aspect-[2/3] bg-muted rounded-lg flex items-center justify-center overflow-hidden">
          <ItemIcon class="w-24 h-24 text-muted-foreground/30" />
        </div>

        <!-- Action buttons -->
        <div class="flex flex-col gap-2 mt-4">
          <div class="flex gap-2">
            <Button
              variant={userItemData?.is_favorite ? 'default' : 'outline'}
              class="flex-1"
              onclick={handleToggleFavorite}
            >
              <Heart class="w-4 h-4 mr-2 {userItemData?.is_favorite ? 'fill-current' : ''}" />
              Favorite
            </Button>

            {#if userItemData?.played}
              <Button variant="outline" class="flex-1" onclick={handleMarkUnplayed}>
                <Check class="w-4 h-4 mr-2" />
                Unmark
              </Button>
            {:else}
              <Button variant="outline" class="flex-1" onclick={handleMarkPlayed}>
                <Check class="w-4 h-4 mr-2" />
                Mark Played
              </Button>
            {/if}
          </div>
        </div>
      </div>

      <!-- Details -->
      <div class="md:col-span-2">
        <div class="flex items-start gap-4 mb-4">
          <div class="flex-1">
            <h1 class="text-3xl font-bold">{item.name}</h1>

            {#if item.original_title && item.original_title !== item.name}
              <p class="text-lg text-muted-foreground mt-1">{item.original_title}</p>
            {/if}

            {#if item.tagline}
              <p class="text-muted-foreground italic mt-2">{item.tagline}</p>
            {/if}
          </div>
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

        <!-- Conversion section -->
        {#if currentProfile}
          <div class="mb-6 p-4 border rounded-lg bg-card">
            <div class="flex items-center gap-2 mb-3">
              <h2 class="text-lg font-semibold">Profile:</h2>
              <ProfileBadge profile={currentProfile} />
              {#if needsConversion}
                <Badge variant="outline" class="ml-2">
                  <AlertTriangle class="w-3 h-3 mr-1" />
                  Needs Conversion
                </Badge>
              {/if}
            </div>

            {#if loadingConversion}
              <div class="flex items-center text-sm text-muted-foreground">
                <Loader2 class="w-4 h-4 mr-2 animate-spin" />
                Loading conversion options...
              </div>
            {:else if sourceAnalysis && conversionOptions}
              {#if sourceAnalysis.capabilities.length > 0}
                <div class="mb-4">
                  <h3 class="text-sm font-medium mb-2">Source Analysis:</h3>
                  <ul class="text-sm text-muted-foreground space-y-1">
                    <li>{sourceAnalysis.specs} - {sourceAnalysis.capabilities.join(', ')}</li>
                  </ul>
                </div>
              {/if}

              {#if conversionActions.length > 0}
                <div class="flex flex-wrap gap-2">
                  {#each conversionActions as action}
                    <Button
                      variant="default"
                      disabled={converting}
                      onclick={() => handleConvert(action.profiles)}
                    >
                      {#if converting}
                        <Loader2 class="w-4 h-4 mr-2 animate-spin" />
                      {/if}
                      {action.label}
                    </Button>
                  {/each}
                  <Button variant="outline" disabled={converting}>Keep as-is</Button>
                </div>
              {:else if conversionOptions.current_profiles.length > 0}
                <p class="text-sm text-muted-foreground">
                  All viable profiles already exist for this item.
                </p>
              {/if}
            {/if}
          </div>
        {/if}

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
              {#each item.genres as genre}
                <Badge variant="outline">{genre}</Badge>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Studios -->
        {#if item.studios.length > 0}
          <div class="mb-6">
            <h2 class="text-lg font-semibold mb-2">Studios</h2>
            <p class="text-muted-foreground">{item.studios.join(', ')}</p>
          </div>
        {/if}

        <!-- Technical details -->
        <div class="mb-6">
          <h2 class="text-lg font-semibold mb-2">Technical Details</h2>
          <div class="grid grid-cols-2 gap-4 text-sm">
            {#if item.resolution}
              <div>
                <span class="text-muted-foreground">Resolution:</span>
                <span class="ml-2 font-medium">{item.resolution}</span>
              </div>
            {/if}
            {#if item.video_codec}
              <div>
                <span class="text-muted-foreground">Video:</span>
                <span class="ml-2 font-medium uppercase">{item.video_codec}</span>
              </div>
            {/if}
            {#if item.audio_codec}
              <div>
                <span class="text-muted-foreground">Audio:</span>
                <span class="ml-2 font-medium uppercase">{item.audio_codec}</span>
              </div>
            {/if}
            {#if item.container}
              <div>
                <span class="text-muted-foreground">Container:</span>
                <span class="ml-2 font-medium uppercase">{item.container}</span>
              </div>
            {/if}
            {#if item.size_bytes}
              <div>
                <span class="text-muted-foreground">Size:</span>
                <span class="ml-2 font-medium">{api.formatBytes(item.size_bytes)}</span>
              </div>
            {/if}
          </div>
        </div>

        <!-- Versions -->
        {#if mediaFiles.length > 0}
          <div class="mb-6">
            <h2 class="text-lg font-semibold mb-2">Versions</h2>
            <div class="space-y-3">
              {#each mediaFiles as file}
                <div class="p-4 border rounded-lg space-y-2">
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                      <Badge variant={file.serves_as_universal ? 'secondary' : 'default'}>
                        Profile {file.serves_as_universal ? 'B' : 'A'}
                      </Badge>
                      <span class="text-sm font-medium capitalize">{file.role}</span>
                    </div>
                    <span class="text-sm text-muted-foreground">{api.formatBytes(file.file_size)}</span>
                  </div>
                  <div class="grid grid-cols-2 gap-2 text-sm">
                    {#if file.width && file.height}
                      <div>
                        <span class="text-muted-foreground">Resolution:</span>
                        <span class="ml-2">{file.width}x{file.height}</span>
                      </div>
                    {/if}
                    {#if file.video_codec}
                      <div>
                        <span class="text-muted-foreground">Video:</span>
                        <span class="ml-2 uppercase">{file.video_codec}</span>
                      </div>
                    {/if}
                    {#if file.audio_codec}
                      <div>
                        <span class="text-muted-foreground">Audio:</span>
                        <span class="ml-2 uppercase">{file.audio_codec}</span>
                      </div>
                    {/if}
                    {#if file.container}
                      <div>
                        <span class="text-muted-foreground">Container:</span>
                        <span class="ml-2 uppercase">{file.container}</span>
                      </div>
                    {/if}
                  </div>
                  {#if file.is_hdr}
                    <Badge variant="secondary" class="text-xs">HDR</Badge>
                  {/if}
                  <div class="text-xs text-muted-foreground/50 truncate">
                    {file.file_path}
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        <!-- File path (for debug) -->
        {#if item.file_path}
          <div class="text-xs text-muted-foreground/50 truncate mt-8">
            <HardDrive class="w-3 h-3 inline mr-1" />
            {item.file_path}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>
