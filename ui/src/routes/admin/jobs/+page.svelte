<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Progress } from '$lib/components/ui/progress';
  import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
  } from '$lib/components/ui/table';
  import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
  } from '$lib/components/ui/dialog';
  import {
    Activity,
    CheckCircle,
    XCircle,
    Clock,
    RefreshCw,
    ChevronLeft,
    ChevronRight,
    Search,
    Trash2,
    RotateCcw,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    Loader2,
  } from 'lucide-svelte';
  import { goto } from '$app/navigation';
  import {
    getHistory,
    retryJob,
    deleteJob,
    formatJobSource,
    getConversionJobs,
    cancelConversionJob,
  } from '$lib/api';
  import { jobHistory, runningJobs, queuedJobs } from '$lib/stores/jobs.svelte';
  import { subscribe as subscribeToEvents } from '$lib/services/events.svelte';
  import ConversionCard from '$lib/components/ConversionCard.svelte';
  import type { Job, ConversionJob, AppEvent } from '$lib/types';

  let loading = $state(true);
  let error = $state<string | null>(null);
  let globalFilter = $state('');
  let selectedJob = $state<Job | null>(null);
  let sortColumn = $state<string>('completed_at');
  let sortDesc = $state(true);
  let currentPage = $state(0);
  const pageSize = 25;
  let conversionJobs = $state<ConversionJob[]>([]);
  let unsubscribeEvents: (() => void) | null = null;

  // Active conversion jobs (queued or running)
  const activeConversionJobs = $derived(
    conversionJobs.filter(j => j.status === 'queued' || j.status === 'running')
  );

  // Format seconds into human-readable duration
  function formatDuration(secs: number | null | undefined): string {
    if (secs == null || secs <= 0) return '-';
    const s = Math.round(secs);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    const rs = s % 60;
    if (m < 60) return `${m}m ${rs}s`;
    const h = Math.floor(m / 60);
    const rm = m % 60;
    return `${h}h ${rm}m`;
  }

  // Format codec for display
  function formatCodec(codec: string | null): string {
    if (!codec) return '?';
    return codec.toUpperCase();
  }

  // Computed filtered and sorted data
  const filteredJobs = $derived.by(() => {
    let jobs = $jobHistory;

    if (globalFilter) {
      const filter = globalFilter.toLowerCase();
      jobs = jobs.filter(job =>
        job.file_name?.toLowerCase().includes(filter) ||
        job.rule_name?.toLowerCase().includes(filter) ||
        job.status?.toLowerCase().includes(filter)
      );
    }

    jobs = [...jobs].sort((a, b) => {
      let aVal = a[sortColumn as keyof Job];
      let bVal = b[sortColumn as keyof Job];

      if (aVal === null || aVal === undefined) aVal = '';
      if (bVal === null || bVal === undefined) bVal = '';

      if (typeof aVal === 'string' && typeof bVal === 'string') {
        return sortDesc ? bVal.localeCompare(aVal) : aVal.localeCompare(bVal);
      }

      return sortDesc ? (bVal > aVal ? 1 : -1) : (aVal > bVal ? 1 : -1);
    });

    return jobs;
  });

  const totalPages = $derived(Math.ceil(filteredJobs.length / pageSize));
  const paginatedJobs = $derived(filteredJobs.slice(currentPage * pageSize, (currentPage + 1) * pageSize));

  function toggleSort(column: string) {
    if (sortColumn === column) {
      sortDesc = !sortDesc;
    } else {
      sortColumn = column;
      sortDesc = true;
    }
  }

  async function loadData() {
    loading = true;
    error = null;
    try {
      const [history, cjobs] = await Promise.all([
        getHistory(500),
        getConversionJobs().catch(() => [] as ConversionJob[]),
      ]);
      jobHistory.set(history);
      conversionJobs = cjobs;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load jobs';
    } finally {
      loading = false;
    }
  }

  async function handleRetry(job: Job) {
    try {
      await retryJob(job.id);
      await loadData();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to retry job';
    }
  }

  async function handleDelete(job: Job) {
    try {
      await deleteJob(job.id);
      jobHistory.removeJob(job.id);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete job';
    }
  }

  async function handleCancelConversion(jobId: string) {
    try {
      await cancelConversionJob(jobId);
      conversionJobs = conversionJobs.filter(j => j.id !== jobId);
      toast.success('Conversion job cancelled');
    } catch (e) {
      toast.error('Failed to cancel conversion job');
    }
  }

  function handleEvent(event: AppEvent): void {
    if (event.event_type === 'conversion_job_progress') {
      conversionJobs = conversionJobs.map(j =>
        j.id === event.job_id
          ? { ...j, progress_pct: event.progress_pct, encode_fps: event.encode_fps, eta_secs: event.eta_secs, elapsed_secs: event.elapsed_secs, status: 'running' }
          : j
      );
    } else if (event.event_type === 'conversion_job_created') {
      getConversionJobs().then(jobs => { conversionJobs = jobs; }).catch(() => {});
    } else if (event.event_type === 'conversion_job_completed') {
      conversionJobs = conversionJobs.filter(j => j.id !== event.job_id);
      toast.success('Conversion complete!');
    } else if (event.event_type === 'conversion_job_failed') {
      conversionJobs = conversionJobs.map(j =>
        j.id === event.job_id
          ? { ...j, status: 'failed', error_message: event.error }
          : j
      );
      toast.error('Conversion failed: ' + event.error);
    } else if (event.event_type === 'conversion_job_cancelled') {
      conversionJobs = conversionJobs.filter(j => j.id !== event.job_id);
    }
  }

  function getStatusVariant(status: string): 'default' | 'destructive' | 'outline' {
    switch (status) {
      case 'completed':
        return 'default';
      case 'failed':
        return 'destructive';
      default:
        return 'outline';
    }
  }

  function getStatusClass(status: string): string {
    return status === 'completed' ? 'bg-green-500' : '';
  }

  onMount(() => {
    loadData();
    unsubscribeEvents = subscribeToEvents('admin', handleEvent);
  });

  onDestroy(() => {
    if (unsubscribeEvents) {
      unsubscribeEvents();
    }
  });
</script>

<svelte:head>
  <title>Jobs - Admin - Sceneforged</title>
</svelte:head>

<div class="container mx-auto py-6 px-4">
  <!-- Header -->
  <div class="flex items-center justify-between mb-6">
    <div class="flex items-center gap-4">
      <Button variant="ghost" onclick={() => goto('/admin')}>
        <ArrowLeft class="w-4 h-4 mr-2" />
        Dashboard
      </Button>
      <h1 class="text-2xl font-bold">Jobs</h1>
    </div>
    <Button variant="outline" size="sm" onclick={loadData} disabled={loading}>
      <RefreshCw class="h-4 w-4 mr-2 {loading ? 'animate-spin' : ''}" />
      Refresh
    </Button>
  </div>

  {#if error}
    <div class="bg-destructive/10 text-destructive p-4 rounded-md mb-6">
      {error}
    </div>
  {/if}

  <!-- Active Conversion Jobs -->
  {#if activeConversionJobs.length > 0 || $runningJobs.length > 0 || $queuedJobs.length > 0}
    <Card class="mb-6 border-blue-500/50">
      <CardHeader>
        <CardTitle class="flex items-center gap-2">
          <Activity class="h-5 w-5 text-blue-500 animate-pulse" />
          Active Jobs
          <Badge variant="secondary">
            {activeConversionJobs.length + $runningJobs.length + $queuedJobs.length}
          </Badge>
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div class="space-y-4">
          <!-- In-memory running jobs -->
          {#each $runningJobs as job (job.id)}
            <ConversionCard {job} />
          {/each}

          <!-- Conversion jobs with item details -->
          {#each activeConversionJobs as cjob (cjob.id)}
            <div class="space-y-3 p-4 border rounded-lg hover:bg-muted/50 transition-colors">
              <div class="flex items-start justify-between">
                <div class="space-y-1 flex-1 min-w-0">
                  <h3 class="font-semibold text-sm truncate">
                    {cjob.item_name ?? 'Unknown Item'}
                  </h3>
                  <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
                    <span>
                      {formatCodec(cjob.source_video_codec)}/{formatCodec(cjob.source_audio_codec)}
                      {#if cjob.source_resolution}
                        ({cjob.source_resolution})
                      {/if}
                      {#if cjob.source_container}
                        .{cjob.source_container}
                      {/if}
                    </span>
                    <span class="text-muted-foreground/50">&#8594;</span>
                    <span>H264/AAC (1080p) .mp4</span>
                  </div>
                </div>
                <div class="flex items-center gap-2 ml-2">
                  <Badge variant="secondary" class={cjob.status === 'running' ? 'bg-blue-500 text-white' : ''}>
                    {#if cjob.status === 'running'}
                      <Activity class="h-3 w-3 mr-1 animate-pulse" />
                    {:else}
                      <Clock class="h-3 w-3 mr-1" />
                    {/if}
                    {cjob.status}
                  </Badge>
                  <Button
                    variant="ghost"
                    size="sm"
                    class="h-7 w-7 p-0 text-muted-foreground hover:text-destructive"
                    onclick={() => handleCancelConversion(cjob.id)}
                  >
                    <XCircle class="h-4 w-4" />
                  </Button>
                </div>
              </div>
              {#if cjob.status === 'running' || cjob.progress_pct > 0}
                <div class="space-y-1">
                  <div class="flex justify-between text-xs">
                    <span class="text-muted-foreground">
                      {#if cjob.encode_fps}
                        {cjob.encode_fps.toFixed(1)} fps
                      {:else}
                        Encoding...
                      {/if}
                    </span>
                    <span class="font-medium">{cjob.progress_pct.toFixed(1)}%</span>
                  </div>
                  <Progress value={cjob.progress_pct} max={100} />
                  <div class="flex justify-between text-xs text-muted-foreground">
                    <span>Elapsed: {formatDuration(cjob.elapsed_secs)}</span>
                    {#if cjob.eta_secs != null && cjob.eta_secs > 0}
                      <span>ETA: {formatDuration(cjob.eta_secs)}</span>
                    {/if}
                  </div>
                </div>
              {/if}
              {#if cjob.error_message}
                <p class="text-xs text-destructive">{cjob.error_message}</p>
              {/if}
            </div>
          {/each}

          <!-- In-memory queued jobs -->
          {#each $queuedJobs as job (job.id)}
            <div class="flex items-center justify-between p-3 border rounded-lg">
              <div class="flex-1 min-w-0">
                <p class="font-medium text-sm truncate" title={job.file_path}>
                  {job.file_name}
                </p>
                <p class="text-xs text-muted-foreground">
                  Rule: {job.rule_name ?? 'N/A'}
                </p>
              </div>
              <Badge variant="outline">Queued</Badge>
            </div>
          {/each}
        </div>
      </CardContent>
    </Card>
  {/if}

  <!-- Job History -->
  <Card>
    <CardHeader>
      <div class="flex items-center justify-between">
        <CardTitle>Job History</CardTitle>
        <div class="relative w-64">
          <Search class="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search jobs..."
            class="pl-8"
            bind:value={globalFilter}
          />
        </div>
      </div>
    </CardHeader>
    <CardContent>
      {#if loading && filteredJobs.length === 0}
        <div class="flex items-center justify-center py-12">
          <Loader2 class="w-6 h-6 animate-spin text-muted-foreground" />
        </div>
      {:else}
        <div class="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="cursor-pointer select-none" onclick={() => toggleSort('file_name')}>
                  <div class="flex items-center gap-2">
                    File
                    {#if sortColumn === 'file_name'}
                      {#if sortDesc}<ArrowDown class="h-4 w-4" />{:else}<ArrowUp class="h-4 w-4" />{/if}
                    {/if}
                  </div>
                </TableHead>
                <TableHead class="cursor-pointer select-none" onclick={() => toggleSort('status')}>
                  <div class="flex items-center gap-2">
                    Status
                    {#if sortColumn === 'status'}
                      {#if sortDesc}<ArrowDown class="h-4 w-4" />{:else}<ArrowUp class="h-4 w-4" />{/if}
                    {/if}
                  </div>
                </TableHead>
                <TableHead class="cursor-pointer select-none" onclick={() => toggleSort('rule_name')}>
                  <div class="flex items-center gap-2">
                    Rule
                    {#if sortColumn === 'rule_name'}
                      {#if sortDesc}<ArrowDown class="h-4 w-4" />{:else}<ArrowUp class="h-4 w-4" />{/if}
                    {/if}
                  </div>
                </TableHead>
                <TableHead>Source</TableHead>
                <TableHead class="cursor-pointer select-none" onclick={() => toggleSort('completed_at')}>
                  <div class="flex items-center gap-2">
                    Completed
                    {#if sortColumn === 'completed_at'}
                      {#if sortDesc}<ArrowDown class="h-4 w-4" />{:else}<ArrowUp class="h-4 w-4" />{/if}
                    {/if}
                  </div>
                </TableHead>
                <TableHead class="w-24">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {#if paginatedJobs.length === 0}
                <TableRow>
                  <TableCell colspan={6} class="text-center py-8 text-muted-foreground">
                    No jobs found
                  </TableCell>
                </TableRow>
              {:else}
                {#each paginatedJobs as job (job.id)}
                  <TableRow
                    class="cursor-pointer hover:bg-muted/50"
                    onclick={() => selectedJob = job}
                  >
                    <TableCell class="font-medium truncate max-w-xs">
                      {job.file_name}
                    </TableCell>
                    <TableCell>
                      <Badge variant={getStatusVariant(job.status)} class={getStatusClass(job.status)}>
                        {#if job.status === 'completed'}
                          <CheckCircle class="h-3 w-3 mr-1" />
                        {:else if job.status === 'failed'}
                          <XCircle class="h-3 w-3 mr-1" />
                        {:else}
                          <Clock class="h-3 w-3 mr-1" />
                        {/if}
                        {job.status}
                      </Badge>
                    </TableCell>
                    <TableCell>{job.rule_name ?? '-'}</TableCell>
                    <TableCell>{formatJobSource(job.source)}</TableCell>
                    <TableCell class="text-muted-foreground">
                      {job.completed_at ? new Date(job.completed_at).toLocaleString() : '-'}
                    </TableCell>
                    <TableCell>
                      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                      <div role="group" class="flex items-center gap-1" onclick={(e) => e.stopPropagation()}>
                        {#if job.status === 'failed'}
                          <Button
                            variant="ghost"
                            size="icon"
                            onclick={() => handleRetry(job)}
                            title="Retry"
                          >
                            <RotateCcw class="h-4 w-4" />
                          </Button>
                        {/if}
                        <Button
                          variant="ghost"
                          size="icon"
                          onclick={() => handleDelete(job)}
                          title="Delete"
                        >
                          <Trash2 class="h-4 w-4" />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                {/each}
              {/if}
            </TableBody>
          </Table>
        </div>

        <!-- Pagination -->
        {#if filteredJobs.length > 0}
          <div class="flex items-center justify-between mt-4">
            <div class="text-sm text-muted-foreground">
              Showing {currentPage * pageSize + 1}
              to {Math.min((currentPage + 1) * pageSize, filteredJobs.length)}
              of {filteredJobs.length} results
            </div>
            <div class="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onclick={() => currentPage--}
                disabled={currentPage === 0}
              >
                <ChevronLeft class="h-4 w-4" />
              </Button>
              <span class="text-sm">
                Page {currentPage + 1} of {totalPages || 1}
              </span>
              <Button
                variant="outline"
                size="sm"
                onclick={() => currentPage++}
                disabled={currentPage >= totalPages - 1}
              >
                <ChevronRight class="h-4 w-4" />
              </Button>
            </div>
          </div>
        {/if}
      {/if}
    </CardContent>
  </Card>
</div>

<!-- Job Detail Dialog -->
<Dialog open={!!selectedJob} onOpenChange={(open) => !open && (selectedJob = null)}>
  <DialogContent class="max-w-2xl">
    <DialogHeader>
      <DialogTitle>Job Details</DialogTitle>
    </DialogHeader>
    {#if selectedJob}
      <div class="space-y-4">
        <div class="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span class="text-muted-foreground">File:</span>
            <p class="font-medium break-all">{selectedJob.file_path}</p>
          </div>
          <div>
            <span class="text-muted-foreground">Status:</span>
            <p class="font-medium">{selectedJob.status}</p>
          </div>
          <div>
            <span class="text-muted-foreground">Rule:</span>
            <p class="font-medium">{selectedJob.rule_name ?? 'N/A'}</p>
          </div>
          <div>
            <span class="text-muted-foreground">Source:</span>
            <p class="font-medium">{formatJobSource(selectedJob.source)}</p>
          </div>
          <div>
            <span class="text-muted-foreground">Created:</span>
            <p class="font-medium">{new Date(selectedJob.created_at).toLocaleString()}</p>
          </div>
          <div>
            <span class="text-muted-foreground">Completed:</span>
            <p class="font-medium">
              {selectedJob.completed_at ? new Date(selectedJob.completed_at).toLocaleString() : '-'}
            </p>
          </div>
        </div>
        {#if selectedJob.error}
          <div class="bg-destructive/10 text-destructive p-4 rounded-md">
            <span class="font-medium">Error:</span>
            <p class="mt-1 text-sm">{selectedJob.error}</p>
          </div>
        {/if}
      </div>
    {/if}
  </DialogContent>
</Dialog>
