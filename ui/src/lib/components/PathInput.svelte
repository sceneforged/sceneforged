<script lang="ts">
  import { browsePaths, type DirEntry } from '$lib/api';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { FolderOpen, Check, X, Plus, Trash2, Loader2 } from 'lucide-svelte';

  interface Props {
    paths?: string[];
    placeholder?: string;
  }

  let { paths = $bindable([]), placeholder = '/media/movies' }: Props = $props();

  let inputValue = $state('');
  let suggestions = $state<DirEntry[]>([]);
  let loading = $state(false);
  let showSuggestions = $state(false);
  let validating = $state<Record<string, boolean | null>>({});

  // Debounced autocomplete
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;

  async function handleInput() {
    clearTimeout(debounceTimer);
    if (!inputValue || inputValue.length < 2) {
      suggestions = [];
      return;
    }

    debounceTimer = setTimeout(async () => {
      loading = true;
      try {
        // Get parent directory and search term
        const lastSlash = inputValue.lastIndexOf('/');
        const dir = lastSlash > 0 ? inputValue.substring(0, lastSlash) : '/';
        const search = inputValue.substring(lastSlash + 1);
        suggestions = await browsePaths(dir, search || undefined);
        showSuggestions = true;
      } catch {
        suggestions = [];
      } finally {
        loading = false;
      }
    }, 200);
  }

  function selectSuggestion(entry: DirEntry) {
    inputValue = entry.path;
    showSuggestions = false;
    suggestions = [];
  }

  function addPath() {
    if (inputValue && !paths.includes(inputValue)) {
      paths = [...paths, inputValue];
      validatePath(inputValue);
      inputValue = '';
    }
  }

  function removePath(index: number) {
    const removed = paths[index];
    paths = paths.filter((_, i) => i !== index);
    // Clean up validation state
    const newValidating = { ...validating };
    delete newValidating[removed];
    validating = newValidating;
  }

  async function validatePath(path: string) {
    validating = { ...validating, [path]: null }; // loading
    try {
      await browsePaths(path);
      validating = { ...validating, [path]: true }; // valid directory
    } catch {
      validating = { ...validating, [path]: false }; // invalid
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      addPath();
    }
  }

  function handleBlur() {
    setTimeout(() => {
      showSuggestions = false;
    }, 200);
  }

  // Validate existing paths on mount
  $effect(() => {
    for (const path of paths) {
      if (validating[path] === undefined) {
        validatePath(path);
      }
    }
  });
</script>

<!-- Path list -->
{#if paths.length > 0}
  <div class="space-y-2 mb-3">
    {#each paths as path, i}
      <div class="flex items-center gap-2 p-2 rounded border bg-[var(--muted)]/30">
        <FolderOpen class="h-4 w-4 text-[var(--muted-foreground)] shrink-0" />
        <span class="flex-1 text-sm font-mono truncate">{path}</span>
        {#if validating[path] === null}
          <Loader2 class="h-4 w-4 animate-spin text-[var(--muted-foreground)]" />
        {:else if validating[path] === true}
          <Check class="h-4 w-4 text-green-500" />
        {:else if validating[path] === false}
          <X class="h-4 w-4 text-[var(--destructive)]" />
        {/if}
        <Button variant="ghost" size="icon" class="h-6 w-6" onclick={() => removePath(i)}>
          <Trash2 class="h-3 w-3" />
        </Button>
      </div>
    {/each}
  </div>
{/if}

<!-- Add path input -->
<div class="relative">
  <div class="flex gap-2">
    <div class="relative flex-1">
      <Input
        bind:value={inputValue}
        {placeholder}
        oninput={handleInput}
        onfocus={() => inputValue && handleInput()}
        onblur={handleBlur}
        onkeydown={handleKeydown}
      />
      {#if loading}
        <Loader2 class="absolute right-2 top-2.5 h-4 w-4 animate-spin text-[var(--muted-foreground)]" />
      {/if}
    </div>
    <Button variant="outline" onclick={addPath} disabled={!inputValue}>
      <Plus class="h-4 w-4" />
    </Button>
  </div>

  <!-- Suggestions dropdown -->
  {#if showSuggestions && suggestions.length > 0}
    <div class="absolute z-50 w-full mt-1 bg-[var(--popover)] border rounded-md shadow-lg max-h-48 overflow-auto">
      {#each suggestions as entry}
        <button
          type="button"
          class="w-full px-3 py-2 text-left text-sm hover:bg-[var(--accent)] flex items-center gap-2"
          onmousedown={() => selectSuggestion(entry)}
        >
          <FolderOpen class="h-4 w-4 text-[var(--muted-foreground)]" />
          <span class="truncate">{entry.name}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
