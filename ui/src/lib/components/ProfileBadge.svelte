<script lang="ts">
  import { cn } from '$lib/utils';
  import type { Profile } from '$lib/types';
  import type { HTMLAttributes } from 'svelte/elements';

  interface Props extends HTMLAttributes<HTMLDivElement> {
    profile: Profile | 'AB';
    class?: string;
  }

  let { profile, class: className, ...restProps }: Props = $props();

  const baseStyles = 'inline-flex items-center justify-center rounded-md px-1.5 py-0.5 text-xs font-semibold text-white';

  const profileStyles: Record<Profile | 'AB', string> = {
    A: 'bg-green-600',
    B: 'bg-blue-600',
    AB: 'bg-gradient-to-r from-green-600 to-blue-600',
    C: 'bg-amber-600',
  };
</script>

<div class={cn(baseStyles, profileStyles[profile], className)} {...restProps}>
  {profile}
</div>
