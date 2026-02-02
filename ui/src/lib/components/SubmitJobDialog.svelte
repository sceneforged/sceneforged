<script lang="ts">
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
  } from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Plus, Loader2, AlertCircle } from 'lucide-svelte';
  import { submitJob } from '$lib/api';
  import { toast } from 'svelte-sonner';

  interface Props {
    onSubmitted?: () => void;
  }

  let { onSubmitted }: Props = $props();

  let open = $state(false);
  let filePath = $state('');
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function handleSubmit() {
    if (!filePath.trim()) {
      error = 'Please enter a file path';
      return;
    }

    loading = true;
    error = null;

    try {
      await submitJob(filePath.trim());
      filePath = '';
      open = false;
      toast.success('Job submitted');
      onSubmitted?.();
    } catch (e) {
      if (e instanceof Error) {
        error = e.message;
      } else {
        error = 'Failed to submit job';
      }
    } finally {
      loading = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !loading) {
      handleSubmit();
    }
  }

  function handleOpenChange(isOpen: boolean) {
    open = isOpen;
    if (!isOpen) {
      filePath = '';
      error = null;
    }
  }
</script>

<Dialog bind:open onOpenChange={handleOpenChange}>
  <DialogTrigger>
    {#snippet children()}
      <Button variant="default" size="sm">
        <Plus class="h-4 w-4 mr-2" />
        Submit Job
      </Button>
    {/snippet}
  </DialogTrigger>
  <DialogContent>
    {#snippet children()}
      <DialogHeader>
        {#snippet children()}
          <DialogTitle>Submit Job</DialogTitle>
          <DialogDescription>
            Enter the full path to a media file to queue it for processing.
          </DialogDescription>
        {/snippet}
      </DialogHeader>

      <div class="space-y-4 py-4">
        <div class="space-y-2">
          <label for="file-path" class="text-sm font-medium">File Path</label>
          <Input
            id="file-path"
            placeholder="/path/to/media/file.mkv"
            bind:value={filePath}
            onkeydown={handleKeydown}
            disabled={loading}
          />
        </div>

        {#if error}
          <div class="flex items-center gap-2 text-sm text-destructive">
            <AlertCircle class="h-4 w-4" />
            <span>{error}</span>
          </div>
        {/if}
      </div>

      <DialogFooter>
        {#snippet children()}
          <Button variant="outline" onclick={() => (open = false)} disabled={loading}>
            Cancel
          </Button>
          <Button onclick={handleSubmit} disabled={loading || !filePath.trim()}>
            {#if loading}
              <Loader2 class="h-4 w-4 mr-2 animate-spin" />
              Submitting...
            {:else}
              Submit
            {/if}
          </Button>
        {/snippet}
      </DialogFooter>
    {/snippet}
  </DialogContent>
</Dialog>
