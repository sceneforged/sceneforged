<script lang="ts">
	import { goto } from '$app/navigation';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle,
		CardDescription
	} from '$lib/components/ui/card/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Film, Loader2, AlertCircle } from '@lucide/svelte';
	import { register } from '$lib/api/index.js';
	import { authStore } from '$lib/stores/auth.svelte.js';

	let code = $state('');
	let username = $state('');
	let password = $state('');
	let loading = $state(false);
	let error = $state<string | null>(null);
	let success = $state(false);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		if (!code.trim() || !username.trim() || !password) {
			error = 'Please fill in all fields';
			return;
		}

		loading = true;
		error = null;

		try {
			const result = await register({
				code: code.trim().toUpperCase(),
				username: username.trim(),
				password
			});
			if (result.success) {
				success = true;
				// Auto-login with the returned token
				if (result.token) {
					document.cookie = `sf_session=${result.token}; path=/; max-age=${60 * 60 * 24 * 30}`;
					await authStore.checkStatus();
					goto('/');
				} else {
					goto('/login');
				}
			}
		} catch (e) {
			if (e instanceof Error) {
				error = e.message;
			} else {
				error = 'Registration failed';
			}
		} finally {
			loading = false;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !loading) {
			handleSubmit(new Event('submit'));
		}
	}
</script>

<div class="flex min-h-screen items-center justify-center p-4">
	<Card class="w-full max-w-md">
		<CardHeader class="text-center">
			<div class="mb-2 flex items-center justify-center gap-2">
				<Film class="h-8 w-8 text-primary" />
				<span class="text-2xl font-bold">SceneForged</span>
			</div>
			<CardTitle>Create Account</CardTitle>
			<CardDescription>Enter your invitation code to register</CardDescription>
		</CardHeader>
		<CardContent>
			<form onsubmit={handleSubmit} class="space-y-4">
				<div class="space-y-2">
					<label for="code" class="text-sm font-medium">Invitation Code</label>
					<Input
						id="code"
						type="text"
						placeholder="ABCD1234"
						bind:value={code}
						onkeydown={handleKeydown}
						disabled={loading}
						class="font-mono tracking-wider uppercase"
					/>
				</div>

				<div class="space-y-2">
					<label for="username" class="text-sm font-medium">Username</label>
					<Input
						id="username"
						type="text"
						placeholder="Choose a username"
						bind:value={username}
						onkeydown={handleKeydown}
						disabled={loading}
						autocomplete="username"
					/>
				</div>

				<div class="space-y-2">
					<label for="password" class="text-sm font-medium">Password</label>
					<Input
						id="password"
						type="password"
						placeholder="Choose a password"
						bind:value={password}
						onkeydown={handleKeydown}
						disabled={loading}
						autocomplete="new-password"
					/>
				</div>

				{#if error}
					<div class="flex items-center gap-2 text-sm text-destructive">
						<AlertCircle class="h-4 w-4" />
						<span>{error}</span>
					</div>
				{/if}

				<Button type="submit" class="w-full" disabled={loading}>
					{#if loading}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
						Creating account...
					{:else}
						Create Account
					{/if}
				</Button>

				<p class="text-center text-sm text-muted-foreground">
					Already have an account?
					<a href="/login" class="text-primary hover:underline">Sign in</a>
				</p>
			</form>
		</CardContent>
	</Card>
</div>
