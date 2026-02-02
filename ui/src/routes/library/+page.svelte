<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { libraryStore, selectedLibrary, totalPages } from '$lib/stores/library';
  import MediaCard from '$lib/components/MediaCard.svelte';
  import Button from '$lib/components/ui/button/button.svelte';
  import Input from '$lib/components/ui/input/input.svelte';
  import * as Select from '$lib/components/ui/select';
  import { Search, ChevronLeft, ChevronRight, Library as LibraryIcon, Loader2 } from 'lucide-svelte';
  import type { Library } from '$lib/types';

  let searchQuery = $state('');
  let searchTimeout: ReturnType<typeof setTimeout> | null = null;

  // Get library_id from URL params
  const urlLibraryId = $derived($page.url.searchParams.get('library_id'));

  onMount(async () => {
    await libraryStore.loadLibraries();

    // If library_id in URL, select it
    if (urlLibraryId) {
      libraryStore.selectLibrary(urlLibraryId);
    }

    // Load items if we have libraries
    if ($libraryStore.libraries.length > 0) {
      const libraryId = urlLibraryId ?? $libraryStore.libraries[0]?.id;
      if (libraryId) {
        libraryStore.selectLibrary(libraryId);
        await libraryStore.loadItems({ libraryId });
      }
    }
  });

  function handleLibraryChange(library: Library | undefined) {
    if (library) {
      libraryStore.selectLibrary(library.id);
      libraryStore.loadItems({ libraryId: library.id });
      // Update URL
      const url = new URL($page.url);
      url.searchParams.set('library_id', library.id);
      goto(url.toString(), { replaceState: true });
    }
  }

  function handleSearch() {
    if (searchTimeout) clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      libraryStore.search(searchQuery);
    }, 300);
  }

  function handleItemClick(itemId: string) {
    goto(`/library/${itemId}`);
  }

  function handlePreviousPage() {
    if ($libraryStore.currentPage > 0) {
      libraryStore.setPage($libraryStore.currentPage - 1);
    }
  }

  function handleNextPage() {
    if ($libraryStore.currentPage < $totalPages - 1) {
      libraryStore.setPage($libraryStore.currentPage + 1);
    }
  }
</script>

<svelte:head>
  <title>Library - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 mb-6">
    <div class="flex items-center gap-3">
      <LibraryIcon class="w-8 h-8 text-primary" />
      <h1 class="text-2xl font-bold">Library</h1>
    </div>

    <!-- Controls -->
    <div class="flex items-center gap-4 w-full sm:w-auto">
      <!-- Library selector -->
      {#if $libraryStore.libraries.length > 0}
        <Select.Root
          type="single"
          value={$selectedLibrary?.id ?? ''}
          onValueChange={(value) => {
            const lib = $libraryStore.libraries.find((l) => l.id === value);
            handleLibraryChange(lib);
          }}
        >
          <Select.Trigger class="w-[180px]">
            <span class="truncate">
              {$selectedLibrary?.name ?? 'Select library'}
            </span>
          </Select.Trigger>
          <Select.Content>
            {#each $libraryStore.libraries as library}
              <Select.Item value={library.id} label={library.name}>{library.name}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}

      <!-- Search -->
      <div class="relative flex-1 sm:flex-none sm:w-64">
        <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
        <Input
          type="search"
          placeholder="Search..."
          class="pl-9"
          bind:value={searchQuery}
          oninput={handleSearch}
        />
      </div>
    </div>
  </div>

  <!-- Content -->
  {#if $libraryStore.loading && $libraryStore.items.length === 0}
    <!-- Loading state -->
    <div class="flex items-center justify-center py-20">
      <Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
    </div>
  {:else if $libraryStore.error}
    <!-- Error state -->
    <div class="text-center py-20">
      <p class="text-destructive">{$libraryStore.error}</p>
      <Button
        variant="outline"
        class="mt-4"
        onclick={() => libraryStore.loadItems()}
      >
        Try Again
      </Button>
    </div>
  {:else if $libraryStore.libraries.length === 0}
    <!-- No libraries configured -->
    <div class="text-center py-20">
      <LibraryIcon class="w-16 h-16 mx-auto text-muted-foreground/30 mb-4" />
      <h2 class="text-lg font-medium text-muted-foreground">No libraries configured</h2>
      <p class="text-sm text-muted-foreground mt-1">
        Create a library in Settings to get started
      </p>
      <Button variant="outline" class="mt-4" onclick={() => goto('/settings')}>
        Go to Settings
      </Button>
    </div>
  {:else if $libraryStore.items.length === 0}
    <!-- Empty state -->
    <div class="text-center py-20">
      <LibraryIcon class="w-16 h-16 mx-auto text-muted-foreground/30 mb-4" />
      <h2 class="text-lg font-medium text-muted-foreground">No items found</h2>
      <p class="text-sm text-muted-foreground mt-1">
        {searchQuery ? 'Try a different search term' : 'Scan your library to add media items'}
      </p>
    </div>
  {:else}
    <!-- Items grid -->
    <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
      {#each $libraryStore.items as item (item.id)}
        <MediaCard
          {item}
          onclick={() => handleItemClick(item.id)}
        />
      {/each}
    </div>

    <!-- Pagination -->
    {#if $totalPages > 1}
      <div class="flex items-center justify-center gap-4 mt-8">
        <Button
          variant="outline"
          size="icon"
          disabled={$libraryStore.currentPage === 0}
          onclick={handlePreviousPage}
        >
          <ChevronLeft class="w-4 h-4" />
        </Button>
        <span class="text-sm text-muted-foreground">
          Page {$libraryStore.currentPage + 1} of {$totalPages}
        </span>
        <Button
          variant="outline"
          size="icon"
          disabled={$libraryStore.currentPage >= $totalPages - 1}
          onclick={handleNextPage}
        >
          <ChevronRight class="w-4 h-4" />
        </Button>
      </div>
    {/if}

    <!-- Item count -->
    <div class="text-center text-sm text-muted-foreground mt-4">
      {$libraryStore.totalCount} item{$libraryStore.totalCount !== 1 ? 's' : ''}
    </div>
  {/if}
</div>
