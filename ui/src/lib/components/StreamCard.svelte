<script lang="ts">
  import { User, Clock } from 'lucide-svelte';
  import ProfileBadge from './ProfileBadge.svelte';
  import type { StreamSession } from '$lib/types';
  import type { Profile } from '$lib/types';

  interface Props {
    stream: StreamSession;
  }

  let { stream }: Props = $props();

  // Format duration in minutes/hours
  const formattedDuration = $derived.by(() => {
    const seconds = stream.duration_seconds;
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);

    if (hours > 0) {
      return `${hours}h ${minutes % 60}m`;
    }
    return `${minutes}m`;
  });

  // Parse profile string to Profile type
  const profile = $derived(stream.profile as Profile);
</script>

<div class="flex items-center justify-between p-4 border rounded-lg hover:bg-muted/50 transition-colors">
  <div class="flex items-center gap-3">
    <div class="p-2 rounded-lg bg-primary/10">
      <User class="w-4 h-4 text-primary" />
    </div>
    <div>
      <p class="text-sm font-medium">{stream.client_ip}</p>
      <p class="text-xs text-muted-foreground">Session {stream.id.slice(0, 8)}</p>
    </div>
  </div>

  <div class="flex items-center gap-3">
    <ProfileBadge {profile} />
    <div class="flex items-center gap-1 text-sm text-muted-foreground">
      <Clock class="w-4 h-4" />
      <span>{formattedDuration}</span>
    </div>
  </div>
</div>
