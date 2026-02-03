<script lang="ts">
  import { cn } from '$lib/utils';

  interface Props {
    class?: string;
  }

  let { class: className }: Props = $props();
</script>

<div class={cn('flex flex-col bg-card rounded-lg overflow-hidden shadow-sm w-full', className)}>
  <!-- Poster placeholder matching aspect-[2/3] -->
  <div class="aspect-[2/3] bg-muted animate-pulse w-full"></div>

  <!-- Content area matching MediaCard's p-3 flex-col gap-1 -->
  <div class="p-3 flex flex-col gap-2">
    <!-- Title line placeholder -->
    <div class="h-4 bg-muted animate-pulse rounded w-3/4"></div>

    <!-- Metadata line placeholder (year, runtime, rating) -->
    <div class="h-3 bg-muted animate-pulse rounded w-1/2"></div>
  </div>
</div>
