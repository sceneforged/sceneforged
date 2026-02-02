<script lang="ts">
  import { Card, CardContent } from '$lib/components/ui/card';
  import { cn } from '$lib/utils';
  import type { ComponentType } from 'svelte';

  interface Props {
    icon: ComponentType;
    label: string;
    value: string | number;
    class?: string;
  }

  let { icon: Icon, label, value, class: className }: Props = $props();
</script>

<Card class={cn('hover:shadow-md transition-shadow', className)}>
  <CardContent class="p-6">
    <div class="flex items-center gap-4">
      <div class="p-3 rounded-lg bg-primary/10">
        <Icon class="w-6 h-6 text-primary" />
      </div>
      <div class="flex-1">
        <p class="text-2xl font-bold">{value}</p>
        <p class="text-sm text-muted-foreground">{label}</p>
      </div>
    </div>
  </CardContent>
</Card>
