<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import VersionCard from '$lib/components/VersionCard.svelte';
  import {
    ArrowLeft,
    Film,
    Tv,
    Music,
    FolderOpen,
    Calendar,
    Clock,
    Star,
    Loader2,
    RefreshCw,
    HardDrive,
    Layers,
  } from 'lucide-svelte';
  import * as api from '$lib/api';
  import type { Item, MediaFile } from '$lib/types';

  const itemId = $derived($page.params.itemId!);

  let item = $state<Item | null>(null);
  let mediaFiles = $state<MediaFile[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let converting = $state(false);

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
        return FolderOpen;
    }
  });

  // Check if a universal version exists
  const hasUniversal = $derived(mediaFiles.some(f => f.serves_as_universal));

  // Check if source exists and can create universal
  const hasSource = $derived(mediaFiles.some(f => f.role === 'source'));
  const canCreateUniversal = $derived.by(() => {
    const sourceFile = mediaFiles.find(f => f.role === 'source');
    return sourceFile && sourceFile.can_be_profile_b && !hasUniversal;
  });

  async function loadData() {
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

  async function handleConvert() {
    if (!item || converting) return;
    converting = true;

    try {
      const response = await api.convertItem(item.id, ['B']);
      toast.success(`Universal copy conversion started (Job ID: ${response.job_ids[0]})`);

      // Reload data after starting conversion
      await loadData();
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to start conversion';
      toast.error(message);
    } finally {
      converting = false;
    }
  }

  async function handleRefresh() {
    await loadData();
  }

  onMount(() => {
    loadData();
  });
</script>

<svelte:head>
  <title>{item?.name ?? 'Item Detail'} - Admin - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <!-- Header with back button -->
  <div class="flex items-center justify-between mb-6">
    <Button variant="ghost" onclick={() => goto('/admin')}>
      <ArrowLeft class="w-4 h-4 mr-2" />
      Back to Admin
    </Button>
    <Button variant="outline" size="sm" onclick={handleRefresh} disabled={loading}>
      <RefreshCw class="h-4 w-4 mr-2 {loading ? 'animate-spin' : ''}" />
      Refresh
    </Button>
  </div>

  {#if loading && !item}
    <div class="flex items-center justify-center py-20">
      <Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
    </div>
  {:else if error || !item}
    <div class="text-center py-20">
      <p class="text-destructive mb-4">{error ?? 'Item not found'}</p>
      <Button variant="outline" onclick={() => goto('/admin')}>
        Return to Admin
      </Button>
    </div>
  {:else}
    <!-- Item Header -->
    <Card class="mb-6">
      <CardHeader>
        <div class="flex items-start gap-4">
          <!-- Icon placeholder -->
          <div class="w-16 h-16 rounded-lg bg-muted flex items-center justify-center flex-shrink-0">
            {#if ItemIcon}
              {@const Icon = ItemIcon}
              <Icon class="w-8 h-8 text-muted-foreground" />
            {/if}
          </div>

          <div class="flex-1 min-w-0">
            <CardTitle class="text-2xl mb-2">{item.name}</CardTitle>

            {#if item.original_title && item.original_title !== item.name}
              <p class="text-muted-foreground mb-2">{item.original_title}</p>
            {/if}

            <!-- Metadata badges -->
            <div class="flex flex-wrap gap-2">
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

              <Badge variant="outline" class="capitalize">{item.item_kind}</Badge>
            </div>
          </div>
        </div>
      </CardHeader>

      {#if item.overview}
        <CardContent>
          <p class="text-muted-foreground leading-relaxed">{item.overview}</p>
        </CardContent>
      {/if}
    </Card>

    <!-- Technical Details -->
    <Card class="mb-6">
      <CardHeader>
        <CardTitle class="flex items-center gap-2 text-lg">
          <HardDrive class="w-5 h-5" />
          Technical Details
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div class="grid grid-cols-2 sm:grid-cols-4 gap-4 text-sm">
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

        {#if item.file_path}
          <div class="mt-4 pt-4 border-t text-xs text-muted-foreground/70 flex items-center gap-2">
            <HardDrive class="w-3 h-3 flex-shrink-0" />
            <span class="truncate" title={item.file_path}>{item.file_path}</span>
          </div>
        {/if}
      </CardContent>
    </Card>

    <!-- Versions/Media Files Section -->
    <Card class="mb-6">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="flex items-center gap-2 text-lg">
            <Layers class="w-5 h-5" />
            Versions
            {#if mediaFiles.length > 0}
              <Badge variant="outline">{mediaFiles.length}</Badge>
            {/if}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        {#if mediaFiles.length === 0}
          <div class="text-center py-8 text-muted-foreground">
            <Layers class="w-12 h-12 mx-auto mb-2 opacity-50" />
            <p>No media files found for this item</p>
          </div>
        {:else}
          <div class="space-y-4">
            {#each mediaFiles as mediaFile (mediaFile.id)}
              <VersionCard
                {mediaFile}
                {hasUniversal}
                {converting}
                onConvert={handleConvert}
              />
            {/each}
          </div>
        {/if}
      </CardContent>
    </Card>

    <!-- Create Universal Copy action at bottom if applicable -->
    {#if canCreateUniversal}
      <Card>
        <CardContent class="p-6">
          <div class="flex items-center justify-between">
            <div>
              <h3 class="font-semibold mb-1">No Universal Version Available</h3>
              <p class="text-sm text-muted-foreground">
                Create a web-playable universal copy for broader device compatibility.
              </p>
            </div>
            <Button
              variant="default"
              disabled={converting}
              onclick={handleConvert}
            >
              {#if converting}
                <Loader2 class="w-4 h-4 mr-2 animate-spin" />
              {/if}
              Create Universal Copy
            </Button>
          </div>
        </CardContent>
      </Card>
    {/if}
  {/if}
</div>
