<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
  } from '$lib/components/ui/table';
  import {
    Library,
    HardDrive,
    Radio,
    Clock,
    Activity,
    RefreshCw,
    Pause,
    Loader2,
    CheckCircle,
    XCircle,
    History,
    FolderOpen,
    Settings,
    ExternalLink,
  } from 'lucide-svelte';
  import { getAdminDashboard, formatBytes, batchConvert, getHistory, formatJobSource } from '$lib/api';
  import { runningJobs, queuedJobs, jobHistory, activeJobs } from '$lib/stores/jobs.svelte';
  import { subscribe as subscribeToEvents } from '$lib/services/events.svelte';
  import StatsCard from '$lib/components/StatsCard.svelte';
  import StreamCard from '$lib/components/StreamCard.svelte';
  import ConversionCard from '$lib/components/ConversionCard.svelte';
  import type { DashboardResponse, Job, AppEvent } from '$lib/types';

  let loading = $state(true);
  let error = $state<string | null>(null);
  let data = $state<DashboardResponse | null>(null);
  let recentJobs = $state<Job[]>([]);
  let refreshInterval: ReturnType<typeof setInterval> | null = null;
  let unsubscribeEvents: (() => void) | null = null;

  // Batch conversion state
  let selectedStreamIds = $state<SvelteSet<string>>(new SvelteSet());
  let targetProfile = $state<'A' | 'B' | 'C'>('B');
  let converting = $state(false);

  // Format large numbers with commas
  function formatNumber(num: number): string {
    return num.toLocaleString();
  }

  // Format date for display
  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '-';
    return new Date(dateStr).toLocaleString();
  }

  // Get status badge info
  function getStatusBadge(status: string) {
    switch (status) {
      case 'completed':
        return { variant: 'default' as const, icon: CheckCircle, class: 'bg-green-500' };
      case 'failed':
        return { variant: 'destructive' as const, icon: XCircle, class: '' };
      case 'running':
        return { variant: 'secondary' as const, icon: Activity, class: 'bg-blue-500' };
      case 'queued':
        return { variant: 'outline' as const, icon: Clock, class: '' };
      default:
        return { variant: 'outline' as const, icon: Clock, class: '' };
    }
  }

  async function loadData() {
    try {
      error = null;
      const [dashboardData, historyData] = await Promise.all([
        getAdminDashboard(),
        getHistory(10)
      ]);
      data = dashboardData;
      recentJobs = historyData;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load dashboard';
    } finally {
      loading = false;
    }
  }

  async function handleRefresh() {
    loading = true;
    // Also refresh the jobs stores
    await Promise.all([
      loadData(),
      activeJobs.refresh(),
      jobHistory.refresh(10)
    ]);
  }

  // Handle admin events for real-time updates
  function handleAdminEvent(event: AppEvent): void {
    // Refresh dashboard stats when jobs complete
    if (event.type === 'job:completed' || event.type === 'job:failed') {
      // Update recent jobs list
      if (event.type === 'job:completed') {
        recentJobs = [event.job, ...recentJobs].slice(0, 10);
      }
      // Refresh stats (debounced via interval)
    }

    // Refresh on library changes
    if (event.type.startsWith('library:') || event.type.startsWith('item:')) {
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
      // Deselect all
      selectedStreamIds = new SvelteSet();
    } else {
      // Select all
      selectedStreamIds = new SvelteSet(data.streams.map(s => s.id));
    }
  }

  async function handleBatchConvert() {
    if (!data || selectedStreamIds.size === 0 || converting) return;

    converting = true;
    try {
      // Get item IDs from selected streams
      const selectedStreams = data.streams.filter(s => selectedStreamIds.has(s.id));
      const itemIds = selectedStreams.map(s => String(s.item_id));

      const response = await batchConvert(itemIds, targetProfile);

      toast.success(
        `Batch conversion started: ${response.job_ids.length} job${response.job_ids.length !== 1 ? 's' : ''} created`
      );

      // Clear selection after successful conversion
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

    // Subscribe to admin events for real-time updates
    unsubscribeEvents = subscribeToEvents('admin', handleAdminEvent);

    // Auto-refresh dashboard stats every 30 seconds
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
    <!-- Loading state -->
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
      <StatsCard
        icon={Clock}
        label="Queue"
        value={data.queue.queued + data.queue.running}
      />
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

    <!-- Active Jobs Section -->
    <Card class="mb-6">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="flex items-center gap-2">
            <Activity class="h-5 w-5" />
            Active Jobs
            {#if $runningJobs.length > 0}
              <Badge variant="secondary">{$runningJobs.length}</Badge>
            {/if}
          </CardTitle>
          {#if $runningJobs.length > 0}
            <Button variant="outline" size="sm">
              <Pause class="h-4 w-4 mr-2" />
              Pause All
            </Button>
          {/if}
        </div>
      </CardHeader>
      <CardContent>
        {#if $runningJobs.length === 0}
          <div class="text-center py-8 text-muted-foreground">
            <Activity class="h-12 w-12 mx-auto mb-2 opacity-50" />
            <p>No jobs currently processing</p>
          </div>
        {:else}
          <div class="space-y-4">
            {#each $runningJobs as job (job.id)}
              <ConversionCard {job} />
            {/each}
          </div>
        {/if}
      </CardContent>
    </Card>

    <!-- Queue Section -->
    <Card class="mb-6">
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <Clock class="h-5 w-5" />
          Queue
          {#if $queuedJobs.length > 0}
            <Badge variant="outline">{$queuedJobs.length} pending</Badge>
          {/if}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {#if $queuedJobs.length === 0}
          <div class="text-center py-8 text-muted-foreground">
            <Clock class="h-12 w-12 mx-auto mb-2 opacity-50" />
            <p>No items waiting for conversion</p>
          </div>
        {:else}
          <div class="space-y-2">
            {#each $queuedJobs.slice(0, 5) as job (job.id)}
              <div class="flex items-center justify-between p-3 border rounded-lg">
                <div class="flex-1 min-w-0">
                  <p class="font-medium truncate" title={job.file_path}>
                    {job.file_name}
                  </p>
                  <p class="text-sm text-muted-foreground">
                    Rule: {job.rule_name ?? 'N/A'}
                  </p>
                </div>
                <Badge variant="outline">Queued</Badge>
              </div>
            {/each}
            {#if $queuedJobs.length > 5}
              <p class="text-sm text-muted-foreground text-center pt-2">
                And {$queuedJobs.length - 5} more...
              </p>
            {/if}
          </div>
        {/if}
      </CardContent>
    </Card>

    <!-- Recent Jobs Section -->
    <Card class="mb-6">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="flex items-center gap-2">
            <History class="h-5 w-5" />
            Recent Jobs
          </CardTitle>
          <Button variant="ghost" size="sm" href="/history">
            View All
            <ExternalLink class="h-4 w-4 ml-2" />
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {#if recentJobs.length === 0}
          <div class="text-center py-8 text-muted-foreground">
            <History class="h-12 w-12 mx-auto mb-2 opacity-50" />
            <p>No recent jobs</p>
          </div>
        {:else}
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>File</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Rule</TableHead>
                <TableHead>Source</TableHead>
                <TableHead>Completed</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {#each recentJobs as job (job.id)}
                {@const statusInfo = getStatusBadge(job.status)}
                <TableRow>
                  <TableCell class="font-medium truncate max-w-xs">
                    <span title={job.file_path}>{job.file_name}</span>
                  </TableCell>
                  <TableCell>
                    <Badge variant={statusInfo.variant} class={statusInfo.class}>
                      {#if statusInfo.icon}
                        {@const StatusIcon = statusInfo.icon}
                        <StatusIcon class="h-3 w-3 mr-1" />
                      {/if}
                      {job.status}
                    </Badge>
                  </TableCell>
                  <TableCell>{job.rule_name ?? '-'}</TableCell>
                  <TableCell>{formatJobSource(job.source)}</TableCell>
                  <TableCell class="text-muted-foreground">
                    {formatDate(job.completed_at)}
                  </TableCell>
                </TableRow>
              {/each}
            </TableBody>
          </Table>
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
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          <Button variant="outline" class="h-auto py-4 flex-col gap-2" href="/history">
            <History class="h-6 w-6" />
            <span>Job History</span>
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

    <!-- Processing Rules Section (Hardcoded Display) -->
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
