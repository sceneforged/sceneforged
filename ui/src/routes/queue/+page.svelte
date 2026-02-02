<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Progress } from '$lib/components/ui/progress';
  import { Separator } from '$lib/components/ui/separator';
  import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
  } from '$lib/components/ui/table';
  import {
    Clock,
    Activity,
    RefreshCw,
    Trash2
  } from 'lucide-svelte';
  import { getJobs, deleteJob, formatJobSource } from '$lib/api';
  import { activeJobs, queuedJobs, runningJobs, connectToEvents, disconnectFromEvents } from '$lib/stores/jobs';
  import { toast } from 'svelte-sonner';
  import SubmitJobDialog from '$lib/components/SubmitJobDialog.svelte';
  import type { Job } from '$lib/types';

  let loading = $state(true);
  let error = $state<string | null>(null);
  let deletingJobs = $state(new Set<string>());

  async function loadData() {
    loading = true;
    error = null;
    try {
      const jobs = await getJobs();
      activeJobs.set(jobs);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load queue';
    } finally {
      loading = false;
    }
  }

  async function handleDelete(job: Job) {
    deletingJobs = new Set([...deletingJobs, job.id]);
    error = null;
    try {
      await deleteJob(job.id);
      toast.success('Job removed from queue');
      await loadData();
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to delete job';
      error = message;
      toast.error(message);
    } finally {
      deletingJobs = new Set([...deletingJobs].filter((id) => id !== job.id));
    }
  }

  onMount(() => {
    loadData();
    connectToEvents();
  });

  onDestroy(() => {
    disconnectFromEvents();
  });

  function formatTime(dateStr: string | null): string {
    if (!dateStr) return '-';
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    if (diff < 60000) return 'Just now';
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    return date.toLocaleDateString();
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">Queue</h1>
    <div class="flex items-center gap-2">
      <SubmitJobDialog onSubmitted={loadData} />
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

  <!-- Running Jobs Section -->
  <Card>
    <CardHeader>
      <CardTitle class="flex items-center gap-2">
        <Activity class="h-5 w-5" />
        Active Jobs
        {#if $runningJobs.length > 0}
          <Badge variant="secondary">{$runningJobs.length}</Badge>
        {/if}
      </CardTitle>
    </CardHeader>
    <CardContent>
      {#if $runningJobs.length === 0}
        <div class="text-center py-8 text-muted-foreground">
          <Activity class="h-12 w-12 mx-auto mb-2 opacity-50" />
          <p>No jobs currently processing</p>
        </div>
      {:else}
        <div class="space-y-6">
          {#each $runningJobs as job}
            <div class="space-y-3 p-4 border rounded-lg">
              <div class="flex items-start justify-between">
                <div class="space-y-1">
                  <h3 class="font-semibold truncate max-w-md" title={job.file_path}>
                    {job.file_name}
                  </h3>
                  <div class="flex items-center gap-2 text-sm text-muted-foreground">
                    <span>Rule: {job.rule_name ?? 'N/A'}</span>
                    <span>-</span>
                    <span>{formatJobSource(job.source)}</span>
                  </div>
                </div>
                <Badge variant="secondary" class="bg-blue-500 text-white">
                  <Activity class="h-3 w-3 mr-1 animate-pulse" />
                  Running
                </Badge>
              </div>

              <div class="space-y-2">
                <div class="flex justify-between text-sm">
                  <span class="text-muted-foreground">
                    {job.current_step ?? 'Processing...'}
                  </span>
                  <span class="font-medium">{job.progress.toFixed(1)}%</span>
                </div>
                <Progress value={job.progress} max={100} />
              </div>

              <div class="text-xs text-muted-foreground">
                Started: {formatTime(job.started_at)}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </CardContent>
  </Card>

  <!-- Queued Jobs Section -->
  <Card>
    <CardHeader>
      <CardTitle class="flex items-center gap-2">
        <Clock class="h-5 w-5" />
        Queued Jobs
        {#if $queuedJobs.length > 0}
          <Badge variant="outline">{$queuedJobs.length}</Badge>
        {/if}
      </CardTitle>
    </CardHeader>
    <CardContent>
      {#if $queuedJobs.length === 0}
        <div class="text-center py-8 text-muted-foreground">
          <Clock class="h-12 w-12 mx-auto mb-2 opacity-50" />
          <p>No jobs waiting in queue</p>
        </div>
      {:else}
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead class="w-12">#</TableHead>
              <TableHead>File</TableHead>
              <TableHead>Source</TableHead>
              <TableHead>Queued</TableHead>
              <TableHead class="w-20">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {#each $queuedJobs as job, i}
              <TableRow>
                <TableCell class="text-muted-foreground">{i + 1}</TableCell>
                <TableCell>
                  <div class="truncate max-w-xs" title={job.file_path}>
                    {job.file_name}
                  </div>
                </TableCell>
                <TableCell>{formatJobSource(job.source)}</TableCell>
                <TableCell class="text-muted-foreground">
                  {formatTime(job.created_at)}
                </TableCell>
                <TableCell>
                  <Button
                    variant="ghost"
                    size="icon"
                    disabled={deletingJobs.has(job.id)}
                    onclick={() => handleDelete(job)}
                    title="Delete"
                  >
                    <Trash2 class="h-4 w-4 {deletingJobs.has(job.id) ? 'animate-pulse' : ''}" />
                  </Button>
                </TableCell>
              </TableRow>
            {/each}
          </TableBody>
        </Table>
      {/if}
    </CardContent>
  </Card>

  <!-- Summary -->
  <div class="flex items-center justify-center gap-4 text-sm text-muted-foreground">
    <span>{$runningJobs.length} running</span>
    <Separator orientation="vertical" class="h-4" />
    <span>{$queuedJobs.length} queued</span>
    <Separator orientation="vertical" class="h-4" />
    <span>{$runningJobs.length + $queuedJobs.length} total</span>
  </div>
</div>
