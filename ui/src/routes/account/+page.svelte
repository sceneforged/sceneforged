<script lang="ts">
	import { authStore } from '$lib/stores/auth.svelte.js';
	import { changePassword } from '$lib/api/index.js';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle,
		CardDescription
	} from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { UserCircle, Lock, Loader2, CheckCircle, AlertCircle } from '@lucide/svelte';

	let currentPassword = $state('');
	let newPassword = $state('');
	let confirmPassword = $state('');
	let saving = $state(false);
	let success = $state<string | null>(null);
	let error = $state<string | null>(null);

	const validationError = $derived(() => {
		if (newPassword && newPassword.length < 8) return 'Password must be at least 8 characters';
		if (confirmPassword && newPassword !== confirmPassword) return 'Passwords do not match';
		return null;
	});

	const canSubmit = $derived(
		currentPassword.length > 0 &&
			newPassword.length >= 8 &&
			newPassword === confirmPassword &&
			!saving
	);

	async function handleChangePassword() {
		if (!canSubmit) return;
		saving = true;
		error = null;
		success = null;
		try {
			await changePassword(currentPassword, newPassword);
			success = 'Password changed successfully';
			currentPassword = '';
			newPassword = '';
			confirmPassword = '';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to change password';
		} finally {
			saving = false;
		}
	}
</script>

<svelte:head>
	<title>Account - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<h1 class="text-2xl font-bold">Account</h1>

	<!-- Account Info -->
	<Card>
		<CardHeader>
			<CardTitle class="flex items-center gap-2">
				<UserCircle class="h-5 w-5" />
				Account Info
			</CardTitle>
		</CardHeader>
		<CardContent>
			<div class="space-y-3">
				<div class="flex items-center justify-between">
					<span class="text-sm text-muted-foreground">Username</span>
					<span class="font-medium">{authStore.username || 'N/A'}</span>
				</div>
				<div class="flex items-center justify-between">
					<span class="text-sm text-muted-foreground">Role</span>
					{#if authStore.role}
						<Badge variant={authStore.isAdmin ? 'default' : 'secondary'}>
							{authStore.role}
						</Badge>
					{:else}
						<span class="text-sm text-muted-foreground">N/A</span>
					{/if}
				</div>
			</div>
		</CardContent>
	</Card>

	<!-- Change Password -->
	{#if authStore.authEnabled}
		<Card>
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<Lock class="h-5 w-5" />
					Change Password
				</CardTitle>
				<CardDescription>Update your account password</CardDescription>
			</CardHeader>
			<CardContent>
				<form
					class="space-y-4"
					onsubmit={(e) => {
						e.preventDefault();
						handleChangePassword();
					}}
				>
					<div class="space-y-2">
						<label for="current-password" class="text-sm font-medium">Current Password</label>
						<Input
							id="current-password"
							type="password"
							bind:value={currentPassword}
							placeholder="Enter current password"
							autocomplete="current-password"
						/>
					</div>

					<div class="space-y-2">
						<label for="new-password" class="text-sm font-medium">New Password</label>
						<Input
							id="new-password"
							type="password"
							bind:value={newPassword}
							placeholder="At least 8 characters"
							autocomplete="new-password"
						/>
					</div>

					<div class="space-y-2">
						<label for="confirm-password" class="text-sm font-medium">Confirm New Password</label>
						<Input
							id="confirm-password"
							type="password"
							bind:value={confirmPassword}
							placeholder="Confirm new password"
							autocomplete="new-password"
						/>
					</div>

					{#if validationError()}
						<p class="text-sm text-destructive">{validationError()}</p>
					{/if}

					{#if error}
						<div class="flex items-center gap-2 text-sm text-destructive">
							<AlertCircle class="h-4 w-4" />
							<span>{error}</span>
						</div>
					{/if}

					{#if success}
						<div class="flex items-center gap-2 text-sm text-green-600 dark:text-green-400">
							<CheckCircle class="h-4 w-4" />
							<span>{success}</span>
						</div>
					{/if}

					<Button type="submit" disabled={!canSubmit}>
						{#if saving}
							<Loader2 class="mr-2 h-4 w-4 animate-spin" />
							Changing...
						{:else}
							Change Password
						{/if}
					</Button>
				</form>
			</CardContent>
		</Card>
	{/if}
</div>
