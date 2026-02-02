<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
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
  } from 'lucide-svelte';
  import { getHistory, retryJob, deleteJob, formatJobSource } from '$lib/api';
  import { jobHistory, connectToEvents, disconnectFromEvents } from '$lib/stores/jobs';
  import type { Job } from '$lib/types';

  let loading = $state(true);
  let error = $state<string | null>(null);
  let globalFilter = $state('');
  let selectedJob = $state<Job | null>(null);
  let sortColumn = $state<string>('completed_at');
  let sortDesc = $state(true);
  let currentPage = $state(0);
  const pageSize = 25;

  // Computed filtered and sorted data
  const filteredJobs = $derived.by(() => {
    let jobs = $jobHistory;

    // Filter
    if (globalFilter) {
      const filter = globalFilter.toLowerCase();
      jobs = jobs.filter(job =>
        job.file_name?.toLowerCase().includes(filter) ||
        job.rule_name?.toLowerCase().includes(filter) ||
        job.status?.toLowerCase().includes(filter)
      );
    }

    // Sort
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
      const history = await getHistory(500);
      jobHistory.set(history);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load history';
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
    connectToEvents();
  });

  onDestroy(() => {
    disconnectFromEvents();
  });
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">History</h1>
    <Button variant="outline" size="sm" onclick={loadData} disabled={loading}>
      <RefreshCw class="h-4 w-4 mr-2 {loading ? 'animate-spin' : ''}" />
      Refresh
    </Button>
  </div>

  {#if error}
    <div class="bg-destructive/10 text-destructive p-4 rounded-md">
      {error}
    </div>
  {/if}

  <Card>
    <CardHeader>
      <div class="flex items-center justify-between">
        <CardTitle>Job History</CardTitle>
        <div class="relative w-64">
          <Search class="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search files..."
            class="pl-8"
            bind:value={globalFilter}
          />
        </div>
      </div>
    </CardHeader>
    <CardContent>
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
                  No history found
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
