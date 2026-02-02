<script lang="ts">
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Separator } from '$lib/components/ui/separator';
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '$lib/components/ui/dialog';
  import {
    Settings,
    RefreshCw,
    CheckCircle,
    XCircle,
    AlertTriangle,
    Tv,
    Film,
    HardDrive,
    Activity,
    Wrench,
    FolderOpen,
    Plus,
    Pencil,
    Trash2,
    Loader2,
    AlertCircle
  } from 'lucide-svelte';
  import {
    getTools,
    getHealth,
    testArrConnection,
    getConfigArrs,
    getConfigJellyfins,
    createArr,
    updateArr,
    deleteArr,
    createJellyfin,
    updateJellyfin,
    deleteJellyfin,
    reloadConfig,
    getLibraries,
    createLibrary,
    deleteLibrary,
    scanLibrary,
    type ArrConfigResponse,
    type JellyfinConfigResponse,
    type CreateLibraryRequest,
  } from '$lib/api';
  import type { ToolStatus, HealthResponse, Library } from '$lib/types';
  import { Library as LibraryIcon, Play, Music } from 'lucide-svelte';
  import PathInput from '$lib/components/PathInput.svelte';

  let tools = $state<ToolStatus[]>([]);
  let arrs = $state<ArrConfigResponse[]>([]);
  let jellyfins = $state<JellyfinConfigResponse[]>([]);
  let health = $state<HealthResponse | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let testingArr = $state<string | null>(null);
  let reloading = $state(false);

  // Arr editor state
  let arrEditorOpen = $state(false);
  let editingArr = $state<ArrConfigResponse | null>(null);
  let arrForm = $state({
    name: '',
    type: 'radarr' as 'radarr' | 'sonarr',
    url: '',
    api_key: '',
    enabled: true,
    auto_rescan: true,
    auto_rename: false,
  });
  let arrLoading = $state(false);
  let arrError = $state<string | null>(null);
  let deletingArr = $state<string | null>(null);

  // Jellyfin editor state
  let jellyfinEditorOpen = $state(false);
  let editingJellyfin = $state<JellyfinConfigResponse | null>(null);
  let jellyfinForm = $state({
    name: '',
    url: '',
    api_key: '',
    enabled: true,
  });
  let jellyfinLoading = $state(false);
  let jellyfinError = $state<string | null>(null);
  let deletingJellyfin = $state<string | null>(null);

  // Library state
  let libraries = $state<Library[]>([]);
  let libraryEditorOpen = $state(false);
  let libraryForm = $state({
    name: '',
    media_type: 'movies' as 'movies' | 'tvshows' | 'music',
    paths: [] as string[],
  });
  let libraryLoading = $state(false);
  let libraryError = $state<string | null>(null);
  let deletingLibrary = $state<string | null>(null);
  let scanningLibrary = $state<string | null>(null);

  async function loadData() {
    loading = true;
    error = null;
    try {
      const [toolsData, arrsData, jellyfinsData, healthData, librariesData] = await Promise.all([
        getTools(),
        getConfigArrs(),
        getConfigJellyfins(),
        getHealth(),
        getLibraries(),
      ]);
      tools = toolsData;
      libraries = librariesData;
      arrs = arrsData;
      jellyfins = jellyfinsData;
      health = healthData;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load settings';
    } finally {
      loading = false;
    }
  }

  async function handleTestArr(name: string) {
    testingArr = name;
    try {
      const result = await testArrConnection(name);
      // Update status in UI (temporary until next refresh)
      arrs = arrs.map(a =>
        a.name === name
          ? { ...a, status: result.success ? 'connected' : 'error' }
          : a
      );
      if (result.success) {
        toast.success(`Connected to ${name}`);
      } else {
        toast.error(`Failed to connect to ${name}`);
      }
    } catch (e) {
      toast.error(`Failed to connect to ${name}`);
    } finally {
      testingArr = null;
    }
  }

  async function handleReloadConfig() {
    reloading = true;
    try {
      await reloadConfig();
      await loadData();
      toast.success('Config reloaded');
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to reload config';
      toast.error('Failed to reload config');
    } finally {
      reloading = false;
    }
  }

  // Arr CRUD
  function openArrEditor(arr: ArrConfigResponse | null = null) {
    editingArr = arr;
    if (arr) {
      arrForm = {
        name: arr.name,
        type: arr.type,
        url: arr.url,
        api_key: '',
        enabled: arr.enabled,
        auto_rescan: arr.auto_rescan,
        auto_rename: arr.auto_rename,
      };
    } else {
      arrForm = {
        name: '',
        type: 'radarr',
        url: '',
        api_key: '',
        enabled: true,
        auto_rescan: true,
        auto_rename: false,
      };
    }
    arrError = null;
    arrEditorOpen = true;
  }

  async function handleSaveArr() {
    if (!arrForm.name.trim()) {
      arrError = 'Name is required';
      return;
    }
    if (!arrForm.url.trim()) {
      arrError = 'URL is required';
      return;
    }
    if (!editingArr && !arrForm.api_key.trim()) {
      arrError = 'API key is required';
      return;
    }

    arrLoading = true;
    arrError = null;

    try {
      if (editingArr) {
        const update: any = {
          name: arrForm.name.trim(),
          type: arrForm.type,
          url: arrForm.url.trim(),
          enabled: arrForm.enabled,
          auto_rescan: arrForm.auto_rescan,
          auto_rename: arrForm.auto_rename,
        };
        if (arrForm.api_key) {
          update.api_key = arrForm.api_key;
        }
        await updateArr(editingArr.name, update);
      } else {
        await createArr({
          name: arrForm.name.trim(),
          type: arrForm.type,
          url: arrForm.url.trim(),
          api_key: arrForm.api_key,
          enabled: arrForm.enabled,
          auto_rescan: arrForm.auto_rescan,
          auto_rename: arrForm.auto_rename,
        });
      }
      arrEditorOpen = false;
      await loadData();
      toast.success(`Saved ${arrForm.name}`);
    } catch (e) {
      arrError = e instanceof Error ? e.message : 'Failed to save';
    } finally {
      arrLoading = false;
    }
  }

  async function handleDeleteArr(name: string) {
    if (!confirm(`Delete "${name}"?`)) return;

    deletingArr = name;
    try {
      await deleteArr(name);
      await loadData();
      toast.success(`Deleted ${name}`);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete';
      toast.error(`Failed to delete ${name}`);
    } finally {
      deletingArr = null;
    }
  }

  // Jellyfin CRUD
  function openJellyfinEditor(jellyfin: JellyfinConfigResponse | null = null) {
    editingJellyfin = jellyfin;
    if (jellyfin) {
      jellyfinForm = {
        name: jellyfin.name,
        url: jellyfin.url,
        api_key: '',
        enabled: jellyfin.enabled,
      };
    } else {
      jellyfinForm = {
        name: '',
        url: '',
        api_key: '',
        enabled: true,
      };
    }
    jellyfinError = null;
    jellyfinEditorOpen = true;
  }

  async function handleSaveJellyfin() {
    if (!jellyfinForm.name.trim()) {
      jellyfinError = 'Name is required';
      return;
    }
    if (!jellyfinForm.url.trim()) {
      jellyfinError = 'URL is required';
      return;
    }
    if (!editingJellyfin && !jellyfinForm.api_key.trim()) {
      jellyfinError = 'API key is required';
      return;
    }

    jellyfinLoading = true;
    jellyfinError = null;

    try {
      if (editingJellyfin) {
        const update: any = {
          name: jellyfinForm.name.trim(),
          url: jellyfinForm.url.trim(),
          enabled: jellyfinForm.enabled,
        };
        if (jellyfinForm.api_key) {
          update.api_key = jellyfinForm.api_key;
        }
        await updateJellyfin(editingJellyfin.name, update);
      } else {
        await createJellyfin({
          name: jellyfinForm.name.trim(),
          url: jellyfinForm.url.trim(),
          api_key: jellyfinForm.api_key,
          enabled: jellyfinForm.enabled,
        });
      }
      jellyfinEditorOpen = false;
      await loadData();
      toast.success(`Saved ${jellyfinForm.name}`);
    } catch (e) {
      jellyfinError = e instanceof Error ? e.message : 'Failed to save';
    } finally {
      jellyfinLoading = false;
    }
  }

  async function handleDeleteJellyfin(name: string) {
    if (!confirm(`Delete "${name}"?`)) return;

    deletingJellyfin = name;
    try {
      await deleteJellyfin(name);
      await loadData();
      toast.success(`Deleted ${name}`);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete';
      toast.error(`Failed to delete ${name}`);
    } finally {
      deletingJellyfin = null;
    }
  }

  // Library CRUD
  function openLibraryEditor() {
    libraryForm = {
      name: '',
      media_type: 'movies',
      paths: [],
    };
    libraryError = null;
    libraryEditorOpen = true;
  }

  async function handleSaveLibrary() {
    if (!libraryForm.name.trim()) {
      libraryError = 'Name is required';
      return;
    }
    if (libraryForm.paths.length === 0) {
      libraryError = 'At least one path is required';
      return;
    }

    libraryLoading = true;
    libraryError = null;

    try {
      await createLibrary({
        name: libraryForm.name.trim(),
        media_type: libraryForm.media_type,
        paths: libraryForm.paths,
      });
      libraryEditorOpen = false;
      await loadData();
      toast.success(`Created library ${libraryForm.name}`);
    } catch (e) {
      libraryError = e instanceof Error ? e.message : 'Failed to create library';
    } finally {
      libraryLoading = false;
    }
  }

  async function handleDeleteLibrary(id: string, name: string) {
    if (!confirm(`Delete library "${name}"? This will remove all items from this library.`)) return;

    deletingLibrary = id;
    try {
      await deleteLibrary(id);
      await loadData();
      toast.success(`Deleted library ${name}`);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete library';
      toast.error(`Failed to delete library ${name}`);
    } finally {
      deletingLibrary = null;
    }
  }

  async function handleScanLibrary(id: string) {
    const library = libraries.find(l => l.id === id);
    const libraryName = library?.name ?? 'library';
    scanningLibrary = id;
    try {
      toast.info(`Scanning ${libraryName}...`);
      await scanLibrary(id);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to start scan';
      toast.error(`Failed to scan ${libraryName}`);
    } finally {
      scanningLibrary = null;
    }
  }

  function getMediaTypeIcon(type: string) {
    switch (type) {
      case 'movies': return Film;
      case 'tvshows': return Tv;
      case 'music': return Music;
      default: return LibraryIcon;
    }
  }

  function getToolIcon(name: string) {
    switch (name.toLowerCase()) {
      case 'ffmpeg':
      case 'ffprobe':
        return Film;
      case 'mediainfo':
        return HardDrive;
      case 'mkvmerge':
        return FolderOpen;
      default:
        return Wrench;
    }
  }

  onMount(() => {
    loadData();
  });
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">Settings</h1>
    <div class="flex items-center gap-2">
      <Button variant="outline" size="sm" onclick={handleReloadConfig} disabled={reloading}>
        {#if reloading}
          <Loader2 class="h-4 w-4 mr-2 animate-spin" />
        {:else}
          <RefreshCw class="h-4 w-4 mr-2" />
        {/if}
        Reload Config
      </Button>
      <Button variant="outline" size="sm" onclick={loadData} disabled={loading}>
        <RefreshCw class="h-4 w-4 mr-2 {loading ? 'animate-spin' : ''}" />
        Refresh
      </Button>
    </div>
  </div>

  {#if error}
    <div class="bg-destructive/10 text-destructive p-4 rounded-md">
      {error}
    </div>
  {/if}

  <!-- Server Status -->
  <Card>
    <CardHeader>
      <CardTitle class="flex items-center gap-2">
        <Activity class="h-5 w-5" />
        Server Status
      </CardTitle>
    </CardHeader>
    <CardContent>
      <div class="grid md:grid-cols-3 gap-4">
        <div class="flex items-center gap-3">
          <div class="flex items-center justify-center w-10 h-10 rounded-full bg-green-500/10">
            <CheckCircle class="h-5 w-5 text-green-500" />
          </div>
          <div>
            <p class="font-medium">{health?.status ?? 'Unknown'}</p>
            <p class="text-sm text-muted-foreground">Status</p>
          </div>
        </div>
        <div class="flex items-center gap-3">
          <div class="flex items-center justify-center w-10 h-10 rounded-full bg-primary/10">
            <Settings class="h-5 w-5 text-primary" />
          </div>
          <div>
            <p class="font-medium">v{health?.version ?? '0.0.0'}</p>
            <p class="text-sm text-muted-foreground">Version</p>
          </div>
        </div>
        <div class="flex items-center gap-3">
          <div class="flex items-center justify-center w-10 h-10 rounded-full bg-blue-500/10">
            <Film class="h-5 w-5 text-blue-500" />
          </div>
          <div>
            <p class="font-medium">{health?.stats?.total_processed ?? 0}</p>
            <p class="text-sm text-muted-foreground">Jobs Processed</p>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>

  <!-- External Tools -->
  <Card>
    <CardHeader>
      <CardTitle class="flex items-center gap-2">
        <Wrench class="h-5 w-5" />
        External Tools
      </CardTitle>
      <CardDescription>Required tools for media processing</CardDescription>
    </CardHeader>
    <CardContent>
      {#if tools.length === 0}
        <p class="text-muted-foreground text-center py-4">Loading tools...</p>
      {:else}
        <div class="space-y-3">
          {#each tools as tool}
            {@const Icon = getToolIcon(tool.name)}
            <div class="flex items-center justify-between p-3 rounded-lg border">
              <div class="flex items-center gap-3">
                <div class="flex items-center justify-center w-10 h-10 rounded-lg {tool.available ? 'bg-green-500/10' : 'bg-destructive/10'}">
                  <Icon class="h-5 w-5 {tool.available ? 'text-green-500' : 'text-destructive'}" />
                </div>
                <div>
                  <p class="font-medium">{tool.name}</p>
                  {#if tool.version}
                    <p class="text-xs text-muted-foreground">{tool.version}</p>
                  {/if}
                </div>
              </div>
              <div class="flex items-center gap-2">
                {#if tool.path}
                  <code class="text-xs bg-muted px-2 py-1 rounded hidden md:block">
                    {tool.path}
                  </code>
                {/if}
                {#if tool.available}
                  <Badge variant="default" class="bg-green-500">
                    <CheckCircle class="h-3 w-3 mr-1" />
                    Installed
                  </Badge>
                {:else}
                  <Badge variant="destructive">
                    <XCircle class="h-3 w-3 mr-1" />
                    Missing
                  </Badge>
                {/if}
              </div>
            </div>
          {/each}
        </div>

        {@const missing = tools.filter(t => !t.available)}
        {#if missing.length > 0}
          <div class="mt-4 p-4 bg-amber-500/10 text-amber-700 dark:text-amber-300 rounded-lg">
            <div class="flex items-center gap-2">
              <AlertTriangle class="h-4 w-4" />
              <span class="font-medium">Missing Tools</span>
            </div>
            <p class="text-sm mt-1">
              Install {missing.map(t => t.name).join(', ')} to enable all features.
            </p>
          </div>
        {/if}
      {/if}
    </CardContent>
  </Card>

  <!-- Libraries -->
  <Card>
    <CardHeader>
      <div class="flex items-center justify-between">
        <div>
          <CardTitle class="flex items-center gap-2">
            <LibraryIcon class="h-5 w-5" />
            Libraries
          </CardTitle>
          <CardDescription>Media library paths for scanning</CardDescription>
        </div>
        <Button size="sm" onclick={() => openLibraryEditor()}>
          <Plus class="h-4 w-4 mr-2" />
          Add
        </Button>
      </div>
    </CardHeader>
    <CardContent>
      {#if libraries.length === 0}
        <div class="text-center py-8 text-muted-foreground">
          <LibraryIcon class="h-12 w-12 mx-auto mb-2 opacity-50" />
          <p>No libraries configured</p>
          <p class="text-sm mt-1">Click "Add" to create your first library</p>
        </div>
      {:else}
        <div class="space-y-3">
          {#each libraries as library}
            {@const TypeIcon = getMediaTypeIcon(library.media_type)}
            <div class="flex items-center justify-between p-3 rounded-lg border">
              <div class="flex items-center gap-3">
                <div class="flex items-center justify-center w-10 h-10 rounded-lg bg-primary/10">
                  <TypeIcon class="h-5 w-5 text-primary" />
                </div>
                <div>
                  <p class="font-medium">{library.name}</p>
                  <p class="text-xs text-muted-foreground">{library.media_type} &middot; {library.paths.length} path{library.paths.length !== 1 ? 's' : ''}</p>
                </div>
              </div>
              <div class="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={() => handleScanLibrary(library.id)}
                  disabled={scanningLibrary === library.id}
                >
                  {#if scanningLibrary === library.id}
                    <RefreshCw class="h-4 w-4 animate-spin" />
                  {:else}
                    Scan
                  {/if}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onclick={() => handleDeleteLibrary(library.id, library.name)}
                  disabled={deletingLibrary === library.id}
                >
                  <Trash2 class="h-4 w-4 {deletingLibrary === library.id ? 'animate-pulse' : ''}" />
                </Button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </CardContent>
  </Card>

  <!-- *Arr Integrations -->
  <Card>
    <CardHeader>
      <div class="flex items-center justify-between">
        <div>
          <CardTitle class="flex items-center gap-2">
            <Tv class="h-5 w-5" />
            *Arr Integrations
          </CardTitle>
          <CardDescription>Radarr and Sonarr connections</CardDescription>
        </div>
        <Button size="sm" onclick={() => openArrEditor()}>
          <Plus class="h-4 w-4 mr-2" />
          Add
        </Button>
      </div>
    </CardHeader>
    <CardContent>
      {#if arrs.length === 0}
        <div class="text-center py-8 text-muted-foreground">
          <Tv class="h-12 w-12 mx-auto mb-2 opacity-50" />
          <p>No *arr integrations configured</p>
          <p class="text-sm mt-1">Click "Add" to create one</p>
        </div>
      {:else}
        <div class="space-y-3">
          {#each arrs as arr}
            <div class="flex items-center justify-between p-3 rounded-lg border">
              <div class="flex items-center gap-3">
                <div class="flex items-center justify-center w-10 h-10 rounded-lg bg-primary/10">
                  {#if arr.type === 'radarr'}
                    <Film class="h-5 w-5 text-primary" />
                  {:else}
                    <Tv class="h-5 w-5 text-primary" />
                  {/if}
                </div>
                <div>
                  <p class="font-medium">{arr.name}</p>
                  <p class="text-xs text-muted-foreground">{arr.url}</p>
                </div>
              </div>
              <div class="flex items-center gap-2">
                {#if arr.enabled}
                  <Button
                    variant="outline"
                    size="sm"
                    onclick={() => handleTestArr(arr.name)}
                    disabled={testingArr === arr.name}
                  >
                    {#if testingArr === arr.name}
                      <RefreshCw class="h-4 w-4 animate-spin" />
                    {:else}
                      Test
                    {/if}
                  </Button>
                {:else}
                  <Badge variant="secondary">Disabled</Badge>
                {/if}
                <Button variant="ghost" size="icon" onclick={() => openArrEditor(arr)}>
                  <Pencil class="h-4 w-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onclick={() => handleDeleteArr(arr.name)}
                  disabled={deletingArr === arr.name}
                >
                  <Trash2 class="h-4 w-4 {deletingArr === arr.name ? 'animate-pulse' : ''}" />
                </Button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </CardContent>
  </Card>

  <!-- Jellyfin Integrations -->
  <Card>
    <CardHeader>
      <div class="flex items-center justify-between">
        <div>
          <CardTitle class="flex items-center gap-2">
            <Film class="h-5 w-5" />
            Jellyfin Integrations
          </CardTitle>
          <CardDescription>Media server notifications</CardDescription>
        </div>
        <Button size="sm" onclick={() => openJellyfinEditor()}>
          <Plus class="h-4 w-4 mr-2" />
          Add
        </Button>
      </div>
    </CardHeader>
    <CardContent>
      {#if jellyfins.length === 0}
        <div class="text-center py-8 text-muted-foreground">
          <Film class="h-12 w-12 mx-auto mb-2 opacity-50" />
          <p>No Jellyfin integrations configured</p>
          <p class="text-sm mt-1">Click "Add" to create one</p>
        </div>
      {:else}
        <div class="space-y-3">
          {#each jellyfins as jellyfin}
            <div class="flex items-center justify-between p-3 rounded-lg border">
              <div class="flex items-center gap-3">
                <div class="flex items-center justify-center w-10 h-10 rounded-lg bg-primary/10">
                  <Film class="h-5 w-5 text-primary" />
                </div>
                <div>
                  <p class="font-medium">{jellyfin.name}</p>
                  <p class="text-xs text-muted-foreground">{jellyfin.url}</p>
                </div>
              </div>
              <div class="flex items-center gap-2">
                {#if jellyfin.enabled}
                  <Badge variant="default" class="bg-green-500">Enabled</Badge>
                {:else}
                  <Badge variant="secondary">Disabled</Badge>
                {/if}
                <Button variant="ghost" size="icon" onclick={() => openJellyfinEditor(jellyfin)}>
                  <Pencil class="h-4 w-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onclick={() => handleDeleteJellyfin(jellyfin.name)}
                  disabled={deletingJellyfin === jellyfin.name}
                >
                  <Trash2 class="h-4 w-4 {deletingJellyfin === jellyfin.name ? 'animate-pulse' : ''}" />
                </Button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </CardContent>
  </Card>
</div>

<!-- Arr Editor Dialog -->
<Dialog bind:open={arrEditorOpen}>
  <DialogContent>
    {#snippet children()}
      <DialogHeader>
        {#snippet children()}
          <DialogTitle>{editingArr ? 'Edit Arr' : 'Add Arr'}</DialogTitle>
          <DialogDescription>Configure Radarr or Sonarr integration</DialogDescription>
        {/snippet}
      </DialogHeader>

      <div class="space-y-4 py-4">
        <div class="space-y-2">
          <label for="arr-name" class="text-sm font-medium">Name</label>
          <Input id="arr-name" bind:value={arrForm.name} placeholder="radarr" />
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium">Type</label>
          <div class="flex gap-4">
            <label class="flex items-center gap-2">
              <input type="radio" bind:group={arrForm.type} value="radarr" />
              <span>Radarr</span>
            </label>
            <label class="flex items-center gap-2">
              <input type="radio" bind:group={arrForm.type} value="sonarr" />
              <span>Sonarr</span>
            </label>
          </div>
        </div>

        <div class="space-y-2">
          <label for="arr-url" class="text-sm font-medium">URL</label>
          <Input id="arr-url" bind:value={arrForm.url} placeholder="http://localhost:7878" />
        </div>

        <div class="space-y-2">
          <label for="arr-api-key" class="text-sm font-medium">
            API Key {editingArr ? '(leave empty to keep current)' : ''}
          </label>
          <Input id="arr-api-key" type="password" bind:value={arrForm.api_key} placeholder="Your API key" />
        </div>

        <div class="space-y-2">
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={arrForm.enabled} class="h-4 w-4" />
            <span class="text-sm font-medium">Enabled</span>
          </label>
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={arrForm.auto_rescan} class="h-4 w-4" />
            <span class="text-sm font-medium">Auto Rescan after processing</span>
          </label>
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={arrForm.auto_rename} class="h-4 w-4" />
            <span class="text-sm font-medium">Auto Rename after processing</span>
          </label>
        </div>

        {#if arrError}
          <div class="flex items-center gap-2 text-sm text-destructive">
            <AlertCircle class="h-4 w-4" />
            <span>{arrError}</span>
          </div>
        {/if}
      </div>

      <DialogFooter>
        {#snippet children()}
          <Button variant="outline" onclick={() => arrEditorOpen = false} disabled={arrLoading}>
            Cancel
          </Button>
          <Button onclick={handleSaveArr} disabled={arrLoading}>
            {#if arrLoading}
              <Loader2 class="h-4 w-4 mr-2 animate-spin" />
              Saving...
            {:else}
              Save
            {/if}
          </Button>
        {/snippet}
      </DialogFooter>
    {/snippet}
  </DialogContent>
</Dialog>

<!-- Jellyfin Editor Dialog -->
<Dialog bind:open={jellyfinEditorOpen}>
  <DialogContent>
    {#snippet children()}
      <DialogHeader>
        {#snippet children()}
          <DialogTitle>{editingJellyfin ? 'Edit Jellyfin' : 'Add Jellyfin'}</DialogTitle>
          <DialogDescription>Configure Jellyfin media server</DialogDescription>
        {/snippet}
      </DialogHeader>

      <div class="space-y-4 py-4">
        <div class="space-y-2">
          <label for="jellyfin-name" class="text-sm font-medium">Name</label>
          <Input id="jellyfin-name" bind:value={jellyfinForm.name} placeholder="jellyfin" />
        </div>

        <div class="space-y-2">
          <label for="jellyfin-url" class="text-sm font-medium">URL</label>
          <Input id="jellyfin-url" bind:value={jellyfinForm.url} placeholder="http://localhost:8096" />
        </div>

        <div class="space-y-2">
          <label for="jellyfin-api-key" class="text-sm font-medium">
            API Key {editingJellyfin ? '(leave empty to keep current)' : ''}
          </label>
          <Input id="jellyfin-api-key" type="password" bind:value={jellyfinForm.api_key} placeholder="Your API key" />
        </div>

        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={jellyfinForm.enabled} class="h-4 w-4" />
          <span class="text-sm font-medium">Enabled</span>
        </label>

        {#if jellyfinError}
          <div class="flex items-center gap-2 text-sm text-destructive">
            <AlertCircle class="h-4 w-4" />
            <span>{jellyfinError}</span>
          </div>
        {/if}
      </div>

      <DialogFooter>
        {#snippet children()}
          <Button variant="outline" onclick={() => jellyfinEditorOpen = false} disabled={jellyfinLoading}>
            Cancel
          </Button>
          <Button onclick={handleSaveJellyfin} disabled={jellyfinLoading}>
            {#if jellyfinLoading}
              <Loader2 class="h-4 w-4 mr-2 animate-spin" />
              Saving...
            {:else}
              Save
            {/if}
          </Button>
        {/snippet}
      </DialogFooter>
    {/snippet}
  </DialogContent>
</Dialog>

<!-- Library Editor Dialog -->
<Dialog bind:open={libraryEditorOpen}>
  <DialogContent>
    {#snippet children()}
      <DialogHeader>
        {#snippet children()}
          <DialogTitle>Add Library</DialogTitle>
          <DialogDescription>Configure a media library to scan</DialogDescription>
        {/snippet}
      </DialogHeader>

      <div class="space-y-4 py-4">
        <div class="space-y-2">
          <label for="library-name" class="text-sm font-medium">Name</label>
          <Input id="library-name" bind:value={libraryForm.name} placeholder="My Movies" />
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium">Media Type</label>
          <div class="flex gap-4">
            <label class="flex items-center gap-2">
              <input type="radio" bind:group={libraryForm.media_type} value="movies" />
              <span>Movies</span>
            </label>
            <label class="flex items-center gap-2">
              <input type="radio" bind:group={libraryForm.media_type} value="tvshows" />
              <span>TV Shows</span>
            </label>
            <label class="flex items-center gap-2">
              <input type="radio" bind:group={libraryForm.media_type} value="music" />
              <span>Music</span>
            </label>
          </div>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium">Paths</label>
          <PathInput bind:paths={libraryForm.paths} placeholder="/media/movies" />
          <p class="text-xs text-muted-foreground">Add directories containing your media files</p>
        </div>

        {#if libraryError}
          <div class="flex items-center gap-2 text-sm text-destructive">
            <AlertCircle class="h-4 w-4" />
            <span>{libraryError}</span>
          </div>
        {/if}
      </div>

      <DialogFooter>
        {#snippet children()}
          <Button variant="outline" onclick={() => libraryEditorOpen = false} disabled={libraryLoading}>
            Cancel
          </Button>
          <Button onclick={handleSaveLibrary} disabled={libraryLoading}>
            {#if libraryLoading}
              <Loader2 class="h-4 w-4 mr-2 animate-spin" />
              Creating...
            {:else}
              Create
            {/if}
          </Button>
        {/snippet}
      </DialogFooter>
    {/snippet}
  </DialogContent>
</Dialog>
