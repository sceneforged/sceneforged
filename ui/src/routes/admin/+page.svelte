<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import {
    Library,
    HardDrive,
    Radio,
    Clock,
    Activity,
    RefreshCw,
    Loader2,
    FolderOpen,
    Settings,
    Briefcase,
  } from 'lucide-svelte';
  import { getAdminDashboard, formatBytes, batchConvert, getConversionJobs } from '$lib/api';
  import { runningJobs, queuedJobs, activeJobs } from '$lib/stores/jobs.svelte';
  import { subscribe as subscribeToEvents } from '$lib/services/events.svelte';
  import StatsCard from '$lib/components/StatsCard.svelte';
  import StreamCard from '$lib/components/StreamCard.svelte';
  import type { DashboardResponse, ConversionJob, AppEvent } from '$lib/types';

  let loading = $state(true);
  let error = $state<string | null>(null);
  let data = $state<DashboardResponse | null>(null);
  let conversionJobCount = $state(0);
  let refreshInterval: ReturnType<typeof setInterval> | null = null;
  let unsubscribeEvents: (() => void) | null = null;

  // Batch conversion state
  let selectedStreamIds = $state<SvelteSet<string>>(new SvelteSet());
  let targetProfile = $state<'A' | 'B' | 'C'>('B');
  let converting = $state(false);

  // Total active jobs across both systems
  const totalActiveJobs = $derived(
    $runningJobs.length + $queuedJobs.length + conversionJobCount
  );

  // Format large numbers with commas
  function formatNumber(num: number): string {
    return num.toLocaleString();
  }

  async function loadData() {
    try {
      error = null;
      const [dashboardData, conversionData] = await Promise.all([
        getAdminDashboard(),
        getConversionJobs().catch(() => [] as ConversionJob[]),
      ]);
      data = dashboardData;
      conversionJobCount = conversionData.filter(
        j => j.status === 'queued' || j.status === 'running'
      ).length;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load dashboard';
    } finally {
      loading = false;
    }
  }

  async function handleRefresh() {
    loading = true;
    await Promise.all([
      loadData(),
      activeJobs.refresh(),
    ]);
  }

  // Handle admin events for real-time updates
  function handleAdminEvent(event: AppEvent): void {
    // Refresh stats on job/library changes
    if (
      event.event_type === 'job_completed' ||
      event.event_type === 'job_failed' ||
      event.event_type === 'conversion_job_completed' ||
      event.event_type === 'conversion_job_failed' ||
      event.event_type === 'conversion_job_created' ||
      event.event_type === 'conversion_job_cancelled' ||
      event.event_type.startsWith('library_') ||
      event.event_type.startsWith('item_')
    ) {
      loadData();
    }
  }

  // Batch conversion functions
  function toggleStreamSelection(streamId: string) {
    const newSelected = new SvelteSet(selectedStreamIds);
    if (newSelected.has(streamId)) {
      newSelected.delete(streamId);
    } else {
      newSelected.add(streamId);
    }
    selectedStreamIds = newSelected;
  }

  function toggleSelectAll() {
    if (!data?.streams) return;

    if (selectedStreamIds.size === data.streams.length) {
      selectedStreamIds = new SvelteSet();
    } else {
      selectedStreamIds = new SvelteSet(data.streams.map(s => s.id));
    }
  }

  async function handleBatchConvert() {
    if (!data || selectedStreamIds.size === 0 || converting) return;

    converting = true;
    try {
      const selectedStreams = data.streams.filter(s => selectedStreamIds.has(s.id));
      const itemIds = selectedStreams.map(s => String(s.item_id));

      const response = await batchConvert(itemIds, targetProfile);

      toast.success(
        `Batch conversion started: ${response.job_ids.length} job${response.job_ids.length !== 1 ? 's' : ''} created`
      );

      selectedStreamIds = new SvelteSet();
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to start batch conversion';
      toast.error(message);
    } finally {
      converting = false;
    }
  }

  onMount(async () => {
    await loadData();
    unsubscribeEvents = subscribeToEvents('admin', handleAdminEvent);
    refreshInterval = setInterval(loadData, 30000);
  });

  onDestroy(() => {
    if (refreshInterval) {
      clearInterval(refreshInterval);
    }
    if (unsubscribeEvents) {
      unsubscribeEvents();
    }
  });
</script>

<svelte:head>
  <title>Admin Dashboard - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <!-- Header -->
  <div class="flex items-center justify-between mb-6">
    <h1 class="text-2xl font-bold">Admin Dashboard</h1>
    <Button variant="outline" size="sm" onclick={handleRefresh} disabled={loading}>
      <RefreshCw class="h-4 w-4 mr-2 {loading ? 'animate-spin' : ''}" />
      Refresh
    </Button>
  </div>

  {#if error}
    <div class="bg-destructive/10 text-destructive p-4 rounded-md mb-6">
      {error}
    </div>
  {/if}

  {#if loading && !data}
    <div class="flex items-center justify-center py-20">
      <RefreshCw class="w-8 h-8 animate-spin text-muted-foreground" />
    </div>
  {:else if data}
    <!-- Stats Cards -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
      <StatsCard
        icon={Library}
        label="Library Items"
        value={formatNumber(data.stats.total_items)}
      />
      <StatsCard
        icon={HardDrive}
        label="Storage Used"
        value={formatBytes(data.stats.storage_bytes)}
      />
      <StatsCard
        icon={Radio}
        label="Active Streams"
        value={data.streams.length}
      />
      <a href="/admin/jobs" class="block">
        <StatsCard
          icon={Activity}
          label="Active Jobs"
          value={totalActiveJobs}
        />
      </a>
    </div>

    <!-- Active Streams Section -->
    <Card class="mb-6">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="flex items-center gap-2">
            <Radio class="h-5 w-5" />
            Active Streams
            {#if data.streams.length > 0}
              <Badge variant="secondary">{data.streams.length}</Badge>
            {/if}
            {#if selectedStreamIds.size > 0}
              <Badge variant="outline">{selectedStreamIds.size} selected</Badge>
            {/if}
          </CardTitle>
          {#if data.streams.length > 0}
            <div class="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onclick={toggleSelectAll}
                disabled={converting}
              >
                {selectedStreamIds.size === data.streams.length ? 'Deselect All' : 'Select All'}
              </Button>
              {#if selectedStreamIds.size > 0}
                <div class="flex items-center gap-2">
                  <select
                    bind:value={targetProfile}
                    disabled={converting}
                    class="h-9 rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    <option value="A">Profile A</option>
                    <option value="B">Profile B</option>
                    <option value="C">Profile C</option>
                  </select>
                  <Button
                    variant="default"
                    size="sm"
                    onclick={handleBatchConvert}
                    disabled={converting || selectedStreamIds.size === 0}
                  >
                    {#if converting}
                      <Loader2 class="h-4 w-4 mr-2 animate-spin" />
                    {/if}
                    Convert Selected
                  </Button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </CardHeader>
      <CardContent>
        {#if data.streams.length === 0}
          <div class="text-center py-8 text-muted-foreground">
            <Radio class="h-12 w-12 mx-auto mb-2 opacity-50" />
            <p>No active streaming sessions</p>
          </div>
        {:else}
          <div class="space-y-3">
            {#each data.streams as stream (stream.id)}
              <div class="flex items-center gap-3">
                <input
                  type="checkbox"
                  checked={selectedStreamIds.has(stream.id)}
                  onchange={() => toggleStreamSelection(stream.id)}
                  disabled={converting}
                  class="w-4 h-4 rounded border-input"
                />
                <div class="flex-1">
                  <StreamCard {stream} />
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </CardContent>
    </Card>

    <!-- Quick Links Section -->
    <Card class="mb-6">
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <FolderOpen class="h-5 w-5" />
          Quick Links
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          <Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/admin/libraries">
            <Library class="h-6 w-6" />
            <span>Libraries</span>
          </Button>
          <Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/admin/jobs">
            <Briefcase class="h-6 w-6" />
            <span>Jobs</span>
          </Button>
          <Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/rules">
            <Settings class="h-6 w-6" />
            <span>Rules</span>
          </Button>
          <Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/settings">
            <Settings class="h-6 w-6" />
            <span>Settings</span>
          </Button>
        </div>
      </CardContent>
    </Card>

    <!-- Processing Rules Section -->
    <Card>
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          Processing Rules
          <Badge variant="outline">Read-only</Badge>
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div class="space-y-3">
          <div class="flex items-start gap-3 p-3 border rounded-lg bg-muted/30">
            <Badge variant="secondary" class="mt-0.5">1</Badge>
            <div class="flex-1">
              <h4 class="font-medium mb-1">DV Profile 7 → Profile 8 Conversion</h4>
              <p class="text-sm text-muted-foreground">
                Convert Dolby Vision Profile 7 sources to Profile 8 for universal compatibility
              </p>
            </div>
          </div>

          <div class="flex items-start gap-3 p-3 border rounded-lg bg-muted/30">
            <Badge variant="secondary" class="mt-0.5">2</Badge>
            <div class="flex-1">
              <h4 class="font-medium mb-1">HDR Sources → Universal Profile B</h4>
              <p class="text-sm text-muted-foreground">
                Generate universal HDR10 fallback (Profile B) for HDR sources
              </p>
            </div>
          </div>

          <div class="flex items-start gap-3 p-3 border rounded-lg bg-muted/30">
            <Badge variant="secondary" class="mt-0.5">3</Badge>
            <div class="flex-1">
              <h4 class="font-medium mb-1">SDR Sources → H.264/MP4 Transcode</h4>
              <p class="text-sm text-muted-foreground">
                Transcode SDR sources to H.264/MP4 for maximum compatibility (Profile C)
              </p>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Profile Stats -->
    {#if data.stats.items_by_profile}
      <div class="mt-6 grid grid-cols-1 sm:grid-cols-3 gap-4">
        <Card>
          <CardContent class="p-6">
            <div class="flex items-center justify-between">
              <div>
                <p class="text-sm text-muted-foreground">Profile A Items</p>
                <p class="text-2xl font-bold">{formatNumber(data.stats.items_by_profile.profile_a)}</p>
              </div>
              <div class="px-2 py-1 rounded-md bg-green-600 text-white text-xs font-semibold">
                A
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent class="p-6">
            <div class="flex items-center justify-between">
              <div>
                <p class="text-sm text-muted-foreground">Profile B Items</p>
                <p class="text-2xl font-bold">{formatNumber(data.stats.items_by_profile.profile_b)}</p>
              </div>
              <div class="px-2 py-1 rounded-md bg-blue-600 text-white text-xs font-semibold">
                B
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent class="p-6">
            <div class="flex items-center justify-between">
              <div>
                <p class="text-sm text-muted-foreground">Profile C Items</p>
                <p class="text-2xl font-bold">{formatNumber(data.stats.items_by_profile.profile_c)}</p>
              </div>
              <div class="px-2 py-1 rounded-md bg-amber-600 text-white text-xs font-semibold">
                C
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    {/if}
  {/if}
</div>
