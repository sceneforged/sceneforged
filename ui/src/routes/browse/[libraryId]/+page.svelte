<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import * as api from '$lib/api';
  import type { Library, Item } from '$lib/types';
  import MediaCard from '$lib/components/MediaCard.svelte';
  import Button from '$lib/components/ui/button/button.svelte';
  import Input from '$lib/components/ui/input/input.svelte';
  import { Search, Library as LibraryIcon, Loader2, ArrowLeft } from 'lucide-svelte';

  const libraryId = $derived($page.params.libraryId!);

  let library = $state<Library | null>(null);
  let items = $state<Item[]>([]);
  let totalCount = $state(0);
  let loading = $state(true);
  let loadingMore = $state(false);
  let error = $state<string | null>(null);
  let searchQuery = $state('');
  let searchTimeout: ReturnType<typeof setTimeout> | null = null;
  let initialLoadDone = $state(false);

  const PAGE_SIZE = 24;
  let offset = $state(0);

  const hasMore = $derived(offset + items.length < totalCount);

  onMount(async () => {
    await loadLibraryAndItems();
    initialLoadDone = true;
  });

  // React to search changes (only after initial load)
  $effect(() => {
    // Access searchQuery to track it
    const query = searchQuery;

    if (!initialLoadDone) return;

    if (searchTimeout) clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      // Reset and reload when search changes
      offset = 0;
      items = [];
      loadItems(query);
    }, 300);
  });

  async function loadLibraryAndItems() {
    loading = true;
    error = null;

    try {
      const [libraryData, itemsData] = await Promise.all([
        api.getLibrary(libraryId),
        api.getItems({
          library_id: libraryId,
          limit: PAGE_SIZE,
          offset: 0,
        }),
      ]);

      library = libraryData;
      items = itemsData.items;
      totalCount = itemsData.total_count;
      offset = 0;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load library';
    } finally {
      loading = false;
    }
  }

  async function loadItems(query?: string) {
    loading = items.length === 0;
    error = null;

    try {
      const itemsData = await api.getItems({
        library_id: libraryId,
        search: query || undefined,
        limit: PAGE_SIZE,
        offset: 0,
      });

      items = itemsData.items;
      totalCount = itemsData.total_count;
      offset = 0;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load items';
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    if (loadingMore || !hasMore) return;

    loadingMore = true;

    try {
      const nextOffset = offset + PAGE_SIZE;
      const itemsData = await api.getItems({
        library_id: libraryId,
        search: searchQuery || undefined,
        limit: PAGE_SIZE,
        offset: nextOffset,
      });

      items = [...items, ...itemsData.items];
      offset = nextOffset;
      totalCount = itemsData.total_count;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load more items';
    } finally {
      loadingMore = false;
    }
  }

  function handleItemClick(item: Item) {
    goto(`/browse/${libraryId}/${item.id}`);
  }
</script>

<svelte:head>
  <title>{library?.name ?? 'Browse'} - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <!-- Back button -->
  <Button variant="ghost" class="mb-4" onclick={() => goto('/')}>
    <ArrowLeft class="w-4 h-4 mr-2" />
    Back to Home
  </Button>

  <!-- Header -->
  <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 mb-6">
    <div class="flex items-center gap-3">
      <LibraryIcon class="w-8 h-8 text-primary" />
      <div>
        <h1 class="text-2xl font-bold">{library?.name ?? 'Browse'}</h1>
        {#if !loading && totalCount > 0}
          <p class="text-sm text-muted-foreground">{totalCount} item{totalCount !== 1 ? 's' : ''}</p>
        {/if}
      </div>
    </div>

    <!-- Search -->
    <div class="relative w-full sm:w-64">
      <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
      <Input
        type="search"
        placeholder="Search..."
        class="pl-9"
        bind:value={searchQuery}
      />
    </div>
  </div>

  <!-- Content -->
  {#if loading && items.length === 0}
    <!-- Loading state -->
    <div class="flex items-center justify-center py-20">
      <Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
    </div>
  {:else if error}
    <!-- Error state -->
    <div class="text-center py-20">
      <p class="text-destructive">{error}</p>
      <Button
        variant="outline"
        class="mt-4"
        onclick={() => loadLibraryAndItems()}
      >
        Try Again
      </Button>
    </div>
  {:else if items.length === 0}
    <!-- Empty state -->
    <div class="text-center py-20">
      <LibraryIcon class="w-16 h-16 mx-auto text-muted-foreground/30 mb-4" />
      <h2 class="text-lg font-medium text-muted-foreground">No items found</h2>
      <p class="text-sm text-muted-foreground mt-1">
        {searchQuery ? 'Try a different search term' : 'This library is empty'}
      </p>
    </div>
  {:else}
    <!-- Items grid -->
    <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
      {#each items as item (item.id)}
        <MediaCard
          {item}
          onclick={() => handleItemClick(item)}
        />
      {/each}
    </div>

    <!-- Load more button -->
    {#if hasMore}
      <div class="flex justify-center mt-8">
        <Button
          variant="outline"
          disabled={loadingMore}
          onclick={loadMore}
        >
          {#if loadingMore}
            <Loader2 class="w-4 h-4 mr-2 animate-spin" />
          {/if}
          Load More
        </Button>
      </div>
    {/if}

    <!-- Item count footer -->
    <div class="text-center text-sm text-muted-foreground mt-4">
      Showing {items.length} of {totalCount} item{totalCount !== 1 ? 's' : ''}
    </div>
  {/if}
</div>
