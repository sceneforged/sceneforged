<script lang="ts">
	import { goto } from '$app/navigation';
	import { authStore } from '$lib/stores/auth.svelte.js';
	import { Loader2 } from '@lucide/svelte';

	let { requireAdmin = false, children } = $props<{
		requireAdmin?: boolean;
		children: import('svelte').Snippet;
	}>();

	$effect(() => {
		if (!authStore.initialized) return;
		if (!authStore.authenticated) {
			goto('/login');
		} else if (requireAdmin && !authStore.isAdmin) {
			goto('/');
		}
	});

	const allowed = $derived(
		authStore.initialized &&
			authStore.authenticated &&
			(!requireAdmin || authStore.isAdmin)
	);
</script>

{#if !authStore.initialized}
	<div class="flex items-center justify-center py-20">
		<Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
	</div>
{:else if allowed}
	{@render children()}
{/if}
