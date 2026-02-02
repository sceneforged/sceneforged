<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    placeholder?: string;
    class?: string;
    children?: Snippet;
  }

  let { placeholder = '', class: className, children }: Props = $props();
</script>

<!-- Value component is a placeholder in bits-ui v2 - actual value display
     should be handled by passing children to the Trigger component.
     This component is kept for API compatibility but just renders children or placeholder. -->
<span class={className}>
  {#if children}
    {@render children()}
  {:else}
    <span class="text-muted-foreground">{placeholder}</span>
  {/if}
</span>
