<script lang="ts">
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Input } from '$lib/components/ui/input';
  import {
    Library,
    Plus,
    Trash2,
    RefreshCw,
    Film,
    Tv,
    Music,
    FolderOpen,
    Loader2,
    ScanLine
  } from 'lucide-svelte';
  import { getLibraries, createLibrary, deleteLibrary, scanLibrary } from '$lib/api';
  import type { Library as LibraryType, MediaType } from '$lib/types';
  import PathInput from '$lib/components/PathInput.svelte';

  let libraries = $state<LibraryType[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // New library form
  let showForm = $state(false);
  let formName = $state('');
  let formType = $state<MediaType>('movies');
  let formPaths = $state<string[]>([]);
  let creating = $state(false);
  let scanning = $state<string | null>(null);

  async function loadLibraries() {
    loading = true;
    error = null;
    try {
      libraries = await getLibraries();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load libraries';
    } finally {
      loading = false;
    }
  }

  async function handleCreate() {
    if (!formName.trim() || formPaths.length === 0) {
      toast.error('Name and at least one path are required');
      return;
    }

    creating = true;
    try {
      const newLib = await createLibrary({
        name: formName.trim(),
        media_type: formType,
        paths: formPaths,
      });
      libraries = [...libraries, newLib];
      toast.success(`Library "${newLib.name}" created`);

      // Reset form
      showForm = false;
      formName = '';
      formType = 'movies';
      formPaths = [];
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to create library');
    } finally {
      creating = false;
    }
  }

  async function handleDelete(lib: LibraryType) {
    if (!confirm(`Delete library "${lib.name}"? This will remove all items from the database.`)) {
      return;
    }

    try {
      await deleteLibrary(lib.id);
      libraries = libraries.filter(l => l.id !== lib.id);
      toast.success(`Library "${lib.name}" deleted`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to delete library');
    }
  }

  async function handleScan(lib: LibraryType) {
    scanning = lib.id;
    try {
      await scanLibrary(lib.id);
      toast.success(`Scan started for "${lib.name}"`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to start scan');
    } finally {
      scanning = null;
    }
  }

  function getMediaTypeIcon(type: MediaType) {
    switch (type) {
      case 'movies': return Film;
      case 'tvshows': return Tv;
      case 'music': return Music;
      default: return FolderOpen;
    }
  }

  onMount(loadLibraries);
</script>

<svelte:head>
  <title>Libraries - Admin - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <div class="flex items-center justify-between mb-6">
    <div class="flex items-center gap-3">
      <Library class="w-8 h-8 text-primary" />
      <h1 class="text-2xl font-bold">Libraries</h1>
    </div>
    <div class="flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={loadLibraries} disabled={loading}>
        <RefreshCw class="h-4 w-4 mr-2 {loading ? 'animate-spin' : ''}" />
        Refresh
      </Button>
      <Button size="sm" onclick={() => showForm = !showForm}>
        <Plus class="h-4 w-4 mr-2" />
        Add Library
      </Button>
    </div>
  </div>

  {#if error}
    <div class="bg-destructive/10 text-destructive p-4 rounded-md mb-6">
      {error}
    </div>
  {/if}

  <!-- New Library Form -->
  {#if showForm}
    <Card class="mb-6">
      <CardHeader>
        <CardTitle>New Library</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="space-y-4">
          <div>
            <label class="text-sm font-medium">Name</label>
            <Input bind:value={formName} placeholder="My Movies" />
          </div>

          <div>
            <label class="text-sm font-medium">Type</label>
            <div class="flex gap-4 mt-2">
              <label class="flex items-center gap-2">
                <input type="radio" bind:group={formType} value="movies" />
                <Film class="h-4 w-4" />
                Movies
              </label>
              <label class="flex items-center gap-2">
                <input type="radio" bind:group={formType} value="tvshows" />
                <Tv class="h-4 w-4" />
                TV Shows
              </label>
              <label class="flex items-center gap-2">
                <input type="radio" bind:group={formType} value="music" />
                <Music class="h-4 w-4" />
                Music
              </label>
            </div>
          </div>

          <div>
            <label class="text-sm font-medium">Paths</label>
            <PathInput bind:paths={formPaths} placeholder="/media/movies" />
          </div>

          <div class="flex gap-2">
            <Button onclick={handleCreate} disabled={creating}>
              {#if creating}
                <Loader2 class="h-4 w-4 mr-2 animate-spin" />
              {/if}
              Create Library
            </Button>
            <Button variant="outline" onclick={() => showForm = false}>
              Cancel
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  {/if}

  <!-- Libraries List -->
  {#if loading && libraries.length === 0}
    <div class="flex items-center justify-center py-20">
      <Loader2 class="w-8 h-8 animate-spin text-muted-foreground" />
    </div>
  {:else if libraries.length === 0}
    <Card>
      <CardContent class="py-12 text-center">
        <Library class="w-16 h-16 mx-auto text-muted-foreground/30 mb-4" />
        <h2 class="text-lg font-medium text-muted-foreground">No libraries</h2>
        <p class="text-sm text-muted-foreground mt-1">
          Add a library to start organizing your media.
        </p>
      </CardContent>
    </Card>
  {:else}
    <div class="grid gap-4">
      {#each libraries as lib (lib.id)}
        {@const Icon = getMediaTypeIcon(lib.media_type)}
        <Card>
          <CardContent class="p-4">
            <div class="flex items-start justify-between">
              <div class="flex items-start gap-3">
                <div class="p-2 bg-muted rounded-lg">
                  <Icon class="h-6 w-6 text-primary" />
                </div>
                <div>
                  <h3 class="font-medium">{lib.name}</h3>
                  <div class="flex items-center gap-2 mt-1">
                    <Badge variant="outline">{lib.media_type}</Badge>
                  </div>
                  <div class="text-sm text-muted-foreground mt-2">
                    {#each lib.paths as path}
                      <div class="font-mono text-xs">{path}</div>
                    {/each}
                  </div>
                </div>
              </div>
              <div class="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={() => handleScan(lib)}
                  disabled={scanning === lib.id}
                >
                  {#if scanning === lib.id}
                    <Loader2 class="h-4 w-4 mr-2 animate-spin" />
                  {:else}
                    <ScanLine class="h-4 w-4 mr-2" />
                  {/if}
                  Scan
                </Button>
                <Button
                  variant="destructive"
                  size="sm"
                  onclick={() => handleDelete(lib)}
                >
                  <Trash2 class="h-4 w-4" />
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      {/each}
    </div>
  {/if}
</div>
