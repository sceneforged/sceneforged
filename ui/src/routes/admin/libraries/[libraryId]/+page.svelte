<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { toast } from 'svelte-sonner';
  import { SvelteSet } from 'svelte/reactivity';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import MediaCard from '$lib/components/MediaCard.svelte';
  import {
    ArrowLeft,
    Library as LibraryIcon,
    Loader2,
    RefreshCw,
    CheckSquare,
    Square,
  } from 'lucide-svelte';
  import * as api from '$lib/api';
  import type { Library, Item } from '$lib/types';

  const libraryId = $derived($page.params.libraryId!);

  let library = $state<Library | null>(null);
  let allItems = $state<Item[]>([]);
  let totalCount = $state(0);
  let loading = $state(true);
  let loadingMore = $state(false);
  let error = $state<string | null>(null);

  const PAGE_SIZE = 24;
  let offset = $state(0);

  // Selection state
  let selectedItems = $state<SvelteSet<string>>(new SvelteSet());
  let converting = $state(false);
  let convertingDv = $state(false);

  // Filter state
  type FilterOption = 'all' | 'profile_a_only' | 'profile_c_only' | 'missing_b' | 'dv_profile_7';
  let filterValue = $state<FilterOption>('all');

  const filterOptions: { value: FilterOption; label: string }[] = [
    { value: 'all', label: 'All items' },
    { value: 'profile_a_only', label: 'Profile A only' },
    { value: 'profile_c_only', label: 'Profile C only' },
    { value: 'missing_b', label: 'Missing Profile B' },
    { value: 'dv_profile_7', label: 'Has DV Profile 7' },
  ];

  // Filter items client-side
  const filteredItems = $derived.by(() => {
    switch (filterValue) {
      case 'profile_a_only':
        return allItems.filter(item => item.has_profile_a && !item.has_profile_b);
      case 'profile_c_only':
        return allItems.filter(item => item.has_profile_c && !item.has_profile_b);
      case 'missing_b':
        return allItems.filter(item => !item.has_profile_b);
      case 'dv_profile_7':
        return allItems.filter(item => item.dolby_vision_profile === '7');
      default:
        return allItems;
    }
  });

  const hasMore = $derived(offset + allItems.length < totalCount);

  // Selection helpers
  const selectedCount = $derived(selectedItems.size);
  const allFilteredSelected = $derived(
    filteredItems.length > 0 && filteredItems.every(item => selectedItems.has(item.id))
  );

  // Derived to check if any selected items have DV P7
  const hasDvProfile7Selected = $derived(
    Array.from(selectedItems).some(id => {
      const item = allItems.find(i => i.id === id);
      return item?.dolby_vision_profile === '7';
    })
  );

  onMount(async () => {
    await loadLibraryAndItems();
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
      allItems = itemsData.items;
      totalCount = itemsData.total_count;
      offset = 0;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load library';
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
        limit: PAGE_SIZE,
        offset: nextOffset,
      });

      allItems = [...allItems, ...itemsData.items];
      offset = nextOffset;
      totalCount = itemsData.total_count;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load more items';
    } finally {
      loadingMore = false;
    }
  }

  function toggleSelection(itemId: string) {
    const newSelected = new SvelteSet(selectedItems);
    if (newSelected.has(itemId)) {
      newSelected.delete(itemId);
    } else {
      newSelected.add(itemId);
    }
    selectedItems = newSelected;
  }

  function selectAll() {
    selectedItems = new SvelteSet(filteredItems.map(item => item.id));
  }

  function deselectAll() {
    selectedItems = new SvelteSet();
  }

  async function handleBatchConvert() {
    if (selectedItems.size === 0 || converting) return;

    converting = true;
    try {
      const itemIds = Array.from(selectedItems);
      const response = await api.batchConvert(itemIds, 'B');

      toast.success(
        `Batch conversion started: ${response.job_ids.length} job${response.job_ids.length !== 1 ? 's' : ''} created`
      );

      // Clear selection after successful conversion
      selectedItems = new SvelteSet();
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to start batch conversion';
      toast.error(message);
    } finally {
      converting = false;
    }
  }

  async function handleBatchDvConvert() {
    if (selectedItems.size === 0 || convertingDv) return;

    convertingDv = true;
    try {
      // Filter to only DV P7 items
      const dvItems = Array.from(selectedItems).filter(id => {
        const item = allItems.find(i => i.id === id);
        return item?.dolby_vision_profile === '7';
      });

      if (dvItems.length === 0) {
        toast.error('No DV Profile 7 items selected');
        return;
      }

      const response = await api.batchDvConvert(dvItems);
      toast.success(`DV conversion started: ${response.job_ids.length} job(s) created`);
      selectedItems = new SvelteSet();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to start DV conversion');
    } finally {
      convertingDv = false;
    }
  }

  async function handleRefresh() {
    selectedItems = new SvelteSet();
    await loadLibraryAndItems();
  }
</script>

<svelte:head>
  <title>{library?.name ?? 'Library'} - Admin - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <!-- Back button -->
  <Button variant="ghost" class="mb-4" onclick={() => goto('/admin/libraries')}>
    <ArrowLeft class="w-4 h-4 mr-2" />
    Back to Libraries
  </Button>

  <!-- Header -->
  <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 mb-6">
    <div class="flex items-center gap-3">
      <LibraryIcon class="w-8 h-8 text-primary" />
      <div>
        <h1 class="text-2xl font-bold">{library?.name ?? 'Library'}</h1>
        {#if !loading && totalCount > 0}
          <p class="text-sm text-muted-foreground">
            {totalCount} item{totalCount !== 1 ? 's' : ''}
            {#if filterValue !== 'all'}
              <span class="text-primary">({filteredItems.length} shown)</span>
            {/if}
          </p>
        {/if}
      </div>
    </div>

    <div class="flex items-center gap-2">
      <!-- Filter dropdown -->
      <select
        bind:value={filterValue}
        class="h-9 rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-ring"
      >
        {#each filterOptions as option}
          <option value={option.value}>{option.label}</option>
        {/each}
      </select>

      <Button variant="outline" size="sm" onclick={handleRefresh} disabled={loading}>
        <RefreshCw class="h-4 w-4 mr-2 {loading ? 'animate-spin' : ''}" />
        Refresh
      </Button>
    </div>
  </div>

  <!-- Selection toolbar (sticky when items selected) -->
  {#if selectedCount > 0}
    <div class="sticky top-0 z-10 bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60 border rounded-lg p-4 mb-6 shadow-sm">
      <div class="flex flex-wrap items-center gap-4">
        <div class="flex items-center gap-2">
          <Badge variant="secondary" class="text-sm">
            {selectedCount} selected
          </Badge>
        </div>

        <div class="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onclick={allFilteredSelected ? deselectAll : selectAll}
          >
            {#if allFilteredSelected}
              <Square class="h-4 w-4 mr-2" />
              Deselect All
            {:else}
              <CheckSquare class="h-4 w-4 mr-2" />
              Select All
            {/if}
          </Button>

          <Button
            variant="default"
            size="sm"
            onclick={handleBatchConvert}
            disabled={converting || selectedCount === 0}
          >
            {#if converting}
              <Loader2 class="h-4 w-4 mr-2 animate-spin" />
            {/if}
            Convert to Profile B
          </Button>

          {#if hasDvProfile7Selected}
            <Button
              variant="secondary"
              size="sm"
              onclick={handleBatchDvConvert}
              disabled={convertingDv || selectedCount === 0}
            >
              {#if convertingDv}
                <Loader2 class="h-4 w-4 mr-2 animate-spin" />
              {/if}
              Convert DV → P8
            </Button>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <!-- Content -->
  {#if loading && allItems.length === 0}
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
  {:else if filteredItems.length === 0}
    <!-- Empty state -->
    <div class="text-center py-20">
      <LibraryIcon class="w-16 h-16 mx-auto text-muted-foreground/30 mb-4" />
      <h2 class="text-lg font-medium text-muted-foreground">No items found</h2>
      <p class="text-sm text-muted-foreground mt-1">
        {#if filterValue !== 'all'}
          No items match the current filter. Try a different filter.
        {:else}
          This library is empty.
        {/if}
      </p>
    </div>
  {:else}
    <!-- Items grid with selection checkboxes -->
    <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
      {#each filteredItems as item (item.id)}
        <div class="relative group">
          <!-- Checkbox overlay -->
          <div class="absolute top-2 left-2 z-10">
            <button
              type="button"
              class="w-6 h-6 rounded border-2 flex items-center justify-center transition-colors
                {selectedItems.has(item.id)
                  ? 'bg-primary border-primary text-primary-foreground'
                  : 'bg-background/80 border-muted-foreground/50 hover:border-primary'}
                {converting ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}"
              onclick={(e) => {
                e.stopPropagation();
                if (!converting) toggleSelection(item.id);
              }}
              disabled={converting}
            >
              {#if selectedItems.has(item.id)}
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4">
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              {/if}
            </button>
          </div>

          <MediaCard
            {item}
            {libraryId}
          />
        </div>
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
      Showing {filteredItems.length} of {totalCount} item{totalCount !== 1 ? 's' : ''}
      {#if filterValue !== 'all'}
        (filtered)
      {/if}
    </div>
  {/if}
</div>
