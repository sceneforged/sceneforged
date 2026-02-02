<script lang="ts">
  import { Activity } from 'lucide-svelte';
  import { Progress } from '$lib/components/ui/progress';
  import Badge from '$lib/components/ui/badge/badge.svelte';
  import { formatJobSource } from '$lib/api';
  import type { Job } from '$lib/types';

  interface Props {
    job: Job;
  }

  let { job }: Props = $props();

  // Format time ago
  function formatTimeAgo(dateStr: string | null): string {
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

<div class="space-y-3 p-4 border rounded-lg hover:bg-muted/50 transition-colors">
  <div class="flex items-start justify-between">
    <div class="space-y-1 flex-1 min-w-0">
      <h3 class="font-semibold truncate" title={job.file_path}>
        {job.file_name}
      </h3>
      <div class="flex items-center gap-2 text-sm text-muted-foreground">
        <span>Rule: {job.rule_name ?? 'N/A'}</span>
        <span>-</span>
        <span>{formatJobSource(job.source)}</span>
      </div>
    </div>
    <Badge variant="secondary" class="bg-blue-500 text-white ml-2">
      <Activity class="h-3 w-3 mr-1 animate-pulse" />
      Running
    </Badge>
  </div>

  <div class="space-y-2">
    <div class="flex justify-between text-sm">
      <span class="text-muted-foreground truncate flex-1 min-w-0">
        {job.current_step ?? 'Processing...'}
      </span>
      <span class="font-medium ml-2">{job.progress.toFixed(1)}%</span>
    </div>
    <Progress value={job.progress} max={100} />
  </div>

  <div class="text-xs text-muted-foreground">
    Started: {formatTimeAgo(job.started_at)}
  </div>
</div>
