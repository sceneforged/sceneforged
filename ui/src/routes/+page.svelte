<script lang="ts">
  import { onMount } from 'svelte';
  import { getItems } from '$lib/api';
  import MediaRow from '$lib/components/MediaRow.svelte';
  import type { Item } from '$lib/types';
  import { Loader2 } from 'lucide-svelte';

  let continueWatching = $state<Item[]>([]);
  let recentlyAdded = $state<Item[]>([]);
  let favorites = $state<Item[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function loadData() {
    loading = true;
    error = null;

    try {
      const [continueWatchingRes, recentlyAddedRes, favoritesRes] = await Promise.all([
        getItems({ filter: 'continue_watching', limit: 20 }),
        getItems({ filter: 'recently_added', limit: 20 }),
        getItems({ filter: 'favorites', limit: 20 }),
      ]);

      continueWatching = continueWatchingRes.items;
      recentlyAdded = recentlyAddedRes.items;
      favorites = favoritesRes.items;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load data';
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadData();
  });

  const hasContent = $derived(
    continueWatching.length > 0 || recentlyAdded.length > 0 || favorites.length > 0
  );
</script>

<div class="space-y-8">
  <div>
    <h1 class="text-3xl font-bold text-foreground">Welcome to Sceneforged</h1>
    <p class="text-muted-foreground mt-1">Your personal media library</p>
  </div>

  {#if error}
    <div class="bg-destructive/10 text-destructive p-4 rounded-md">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="flex items-center justify-center py-12">
      <Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
    </div>
  {:else if !hasContent}
    <div class="text-center py-12">
      <p class="text-muted-foreground">
        No media to display yet. Add some libraries in Settings to get started.
      </p>
    </div>
  {:else}
    <div class="space-y-8">
      {#if continueWatching.length > 0}
        <MediaRow title="Continue Watching" items={continueWatching} />
      {/if}

      {#if recentlyAdded.length > 0}
        <MediaRow title="Recently Added" items={recentlyAdded} />
      {/if}

      {#if favorites.length > 0}
        <MediaRow title="Favorites" items={favorites} />
      {/if}
    </div>
  {/if}
</div>
