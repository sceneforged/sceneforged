<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
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
    CheckCircle,
    XCircle,
    Clock,
    Activity,
    Film,
    HardDrive,
    RefreshCw
  } from 'lucide-svelte';
  import { getStats, getJobs, getHistory, formatBytes, formatJobSource } from '$lib/api';
  import { activeJobs, jobHistory, runningJobs, connectToEvents, disconnectFromEvents } from '$lib/stores/jobs';
  import type { Job, JobStats } from '$lib/types';

  let stats = $state<JobStats | null>(null);
  let recentJobs = $state<Job[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function loadData() {
    loading = true;
    error = null;
    try {
      const [statsData, historyData, activeData] = await Promise.all([
        getStats(),
        getHistory(10),
        getJobs()
      ]);
      stats = statsData;
      recentJobs = historyData;
      activeJobs.set(activeData);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load data';
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadData();
    connectToEvents();
  });

  onDestroy(() => {
    disconnectFromEvents();
  });

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

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '-';
    return new Date(dateStr).toLocaleString();
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">Dashboard</h1>
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

  <!-- Stats Cards -->
  <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
    <Card>
      <CardHeader class="flex flex-row items-center justify-between pb-2">
        <CardTitle class="text-sm font-medium">Total Processed</CardTitle>
        <Film class="h-4 w-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        <div class="text-2xl font-bold">{stats?.total_processed ?? 0}</div>
        <p class="text-xs text-muted-foreground">
          All time
        </p>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="flex flex-row items-center justify-between pb-2">
        <CardTitle class="text-sm font-medium">Success Rate</CardTitle>
        <CheckCircle class="h-4 w-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        <div class="text-2xl font-bold">
          {stats ? `${((stats.successful / Math.max(stats.total_processed, 1)) * 100).toFixed(1)}%` : '0%'}
        </div>
        <p class="text-xs text-muted-foreground">
          {stats?.successful ?? 0} successful / {stats?.failed ?? 0} failed
        </p>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="flex flex-row items-center justify-between pb-2">
        <CardTitle class="text-sm font-medium">Active Jobs</CardTitle>
        <Activity class="h-4 w-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        <div class="text-2xl font-bold">{$runningJobs.length}</div>
        <p class="text-xs text-muted-foreground">
          Currently processing
        </p>
      </CardContent>
    </Card>

    <Card>
      <CardHeader class="flex flex-row items-center justify-between pb-2">
        <CardTitle class="text-sm font-medium">Data Processed</CardTitle>
        <HardDrive class="h-4 w-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        <div class="text-2xl font-bold">
          {stats ? formatBytes(stats.total_bytes_processed) : '0 B'}
        </div>
        <p class="text-xs text-muted-foreground">
          Total bytes
        </p>
      </CardContent>
    </Card>
  </div>

  <!-- Active Jobs -->
  {#if $runningJobs.length > 0}
    <Card>
      <CardHeader>
        <CardTitle>Active Jobs</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        {#each $runningJobs as job}
          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <span class="font-medium truncate flex-1 mr-4">{job.file_name}</span>
              <span class="text-sm text-muted-foreground">{job.progress.toFixed(0)}%</span>
            </div>
            <Progress value={job.progress} max={100} />
            {#if job.current_step}
              <p class="text-xs text-muted-foreground">{job.current_step}</p>
            {/if}
          </div>
        {/each}
      </CardContent>
    </Card>
  {/if}

  <!-- Recent Jobs -->
  <Card>
    <CardHeader>
      <CardTitle>Recent Jobs</CardTitle>
    </CardHeader>
    <CardContent>
      {#if recentJobs.length === 0}
        <p class="text-muted-foreground text-center py-8">No recent jobs</p>
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
            {#each recentJobs as job}
              {@const statusInfo = getStatusBadge(job.status)}
              <TableRow>
                <TableCell class="font-medium truncate max-w-xs">
                  {job.file_name}
                </TableCell>
                <TableCell>
                  <Badge variant={statusInfo.variant} class={statusInfo.class}>
                    {@const Icon = statusInfo.icon}
                    <Icon class="h-3 w-3 mr-1" />
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
</div>
