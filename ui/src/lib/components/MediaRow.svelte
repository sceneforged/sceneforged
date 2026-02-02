<script lang="ts">
  import type { Item } from '$lib/types';
  import MediaCard from './MediaCard.svelte';
  import { goto } from '$app/navigation';

  interface Props {
    title: string;
    items: Item[];
  }

  let { title, items }: Props = $props();

  function handleItemClick(item: Item) {
    goto(`/browse/${item.library_id}/${item.id}`);
  }
</script>

<section class="space-y-4">
  <h2 class="text-xl font-semibold text-foreground">{title}</h2>

  <div class="relative">
    <div class="flex gap-4 overflow-x-auto pb-4 scrollbar-thin scrollbar-thumb-muted scrollbar-track-transparent">
      {#each items as item (item.id)}
        <div class="flex-shrink-0 w-40">
          <MediaCard
            {item}
            onclick={() => handleItemClick(item)}
          />
        </div>
      {/each}
    </div>
  </div>
</section>

<style>
  /* Custom scrollbar styles for webkit browsers */
  .scrollbar-thin::-webkit-scrollbar {
    height: 8px;
  }

  .scrollbar-thin::-webkit-scrollbar-track {
    background: transparent;
  }

  .scrollbar-thin::-webkit-scrollbar-thumb {
    background-color: hsl(var(--muted));
    border-radius: 4px;
  }

  .scrollbar-thin::-webkit-scrollbar-thumb:hover {
    background-color: hsl(var(--muted-foreground));
  }
</style>
