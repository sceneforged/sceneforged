<script lang="ts">
	import { cn } from '$lib/utils';
	import { Button } from '$lib/components/ui/button/index.js';
	import { AlertTriangle } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
		class?: string;
	}

	let { children, class: className }: Props = $props();

	let error = $state<Error | null>(null);

	function handleError(event: Event | string, source?: string, lineno?: number, colno?: number, errorObj?: Error) {
		if (errorObj) {
			error = errorObj;
		} else if (event instanceof ErrorEvent) {
			error = event.error instanceof Error ? event.error : new Error(String(event.error));
		} else {
			error = new Error(String(event));
		}
	}

	function reset() {
		error = null;
	}
</script>

<svelte:window onerror={handleError as OnErrorEventHandler} />

{#if error}
	<div class={cn('flex flex-col items-center justify-center gap-4 rounded-lg border border-destructive/50 bg-destructive/5 p-8', className)}>
		<AlertTriangle class="h-12 w-12 text-destructive" />
		<div class="text-center">
			<h3 class="text-lg font-semibold text-destructive">Something went wrong</h3>
			<p class="mt-1 text-sm text-muted-foreground">{error.message}</p>
		</div>
		<Button variant="outline" onclick={reset}>
			Try Again
		</Button>
	</div>
{:else}
	{@render children()}
{/if}
