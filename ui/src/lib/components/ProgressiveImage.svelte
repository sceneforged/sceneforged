<script lang="ts">
  import { cn } from '$lib/utils';

  interface Props {
    imageId: string;
    alt: string;
    size: 'small' | 'medium' | 'large';
    class?: string;
    aspectRatio?: string;
  }

  let { imageId, alt, size, class: className, aspectRatio = '2/3' }: Props = $props();

  let fullLoaded = $state(false);
  let thumbLoaded = $state(false);
  let fullError = $state(false);
  let thumbError = $state(false);

  const thumbSrc = $derived(`/api/images/${imageId}?size=small`);
  const fullSrc = $derived(`/api/images/${imageId}?size=${size}`);

  // Reset load state when imageId or size changes
  $effect(() => {
    // Access derived values to track dependencies
    void thumbSrc;
    void fullSrc;
    fullLoaded = false;
    thumbLoaded = false;
    fullError = false;
    thumbError = false;
  });

  function onThumbLoad() {
    thumbLoaded = true;
  }

  function onThumbError() {
    thumbError = true;
  }

  function onFullLoad() {
    fullLoaded = true;
  }

  function onFullError() {
    fullError = true;
  }
</script>

<div
  class={cn('relative overflow-hidden bg-muted', className)}
  style="aspect-ratio: {aspectRatio};"
>
  <!-- Blurred thumbnail (shown while full image is loading) -->
  {#if !thumbError}
    <img
      src={thumbSrc}
      {alt}
      loading="lazy"
      onload={onThumbLoad}
      onerror={onThumbError}
      class={cn(
        'absolute inset-0 h-full w-full object-cover',
        'blur-lg scale-110',
        'transition-opacity duration-300',
        thumbLoaded && !fullLoaded ? 'opacity-100' : 'opacity-0'
      )}
    />
  {/if}

  <!-- Full-resolution image -->
  {#if !fullError}
    <img
      src={fullSrc}
      {alt}
      loading="lazy"
      onload={onFullLoad}
      onerror={onFullError}
      class={cn(
        'absolute inset-0 h-full w-full object-cover',
        'transition-opacity duration-500 ease-in-out',
        fullLoaded ? 'opacity-100' : 'opacity-0'
      )}
    />
  {/if}
</div>
