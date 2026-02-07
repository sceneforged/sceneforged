<script lang="ts">
	import { onMount } from 'svelte';
	import { listUsers, createUser, updateUser, deleteUser } from '$lib/api/index.js';
	import type { User } from '$lib/types.js';
	import { authStore } from '$lib/stores/auth.svelte.js';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import * as AlertDialog from '$lib/components/ui/alert-dialog/index.js';
	import {
		Users,
		Plus,
		Pencil,
		Trash2,
		Loader2,
		RefreshCw,
		Shield,
		ShieldCheck
	} from '@lucide/svelte';

	let users = $state<User[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Create dialog
	let showCreateDialog = $state(false);
	let newUsername = $state('');
	let newPassword = $state('');
	let newRole = $state('user');
	let creating = $state(false);
	let createError = $state<string | null>(null);

	// Edit dialog
	let showEditDialog = $state(false);
	let editingUser = $state<User | null>(null);
	let editRole = $state('');
	let editPassword = $state('');
	let saving = $state(false);
	let editError = $state<string | null>(null);

	// Delete confirmation
	let showDeleteDialog = $state(false);
	let deletingUser = $state<User | null>(null);
	let deleting = $state(false);

	function isSelf(user: User): boolean {
		return user.id === authStore.userId;
	}

	async function loadUsers() {
		loading = true;
		error = null;
		try {
			users = await listUsers();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load users';
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		if (!newUsername.trim() || !newPassword.trim()) return;
		creating = true;
		createError = null;
		try {
			await createUser({
				username: newUsername.trim(),
				password: newPassword,
				role: newRole
			});
			showCreateDialog = false;
			newUsername = '';
			newPassword = '';
			newRole = 'user';
			await loadUsers();
		} catch (e) {
			createError = e instanceof Error ? e.message : 'Failed to create user';
		} finally {
			creating = false;
		}
	}

	function openEdit(user: User) {
		editingUser = user;
		editRole = user.role;
		editPassword = '';
		editError = null;
		showEditDialog = true;
	}

	async function handleEdit() {
		if (!editingUser) return;
		saving = true;
		editError = null;
		try {
			const data: { role?: string; password?: string } = {};
			if (editRole !== editingUser.role) data.role = editRole;
			if (editPassword.trim()) data.password = editPassword;
			await updateUser(editingUser.id, data);
			showEditDialog = false;
			editingUser = null;
			await loadUsers();
		} catch (e) {
			editError = e instanceof Error ? e.message : 'Failed to update user';
		} finally {
			saving = false;
		}
	}

	function openDelete(user: User) {
		deletingUser = user;
		showDeleteDialog = true;
	}

	async function handleDelete() {
		if (!deletingUser) return;
		deleting = true;
		try {
			await deleteUser(deletingUser.id);
			showDeleteDialog = false;
			deletingUser = null;
			await loadUsers();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete user';
		} finally {
			deleting = false;
		}
	}

	onMount(() => {
		loadUsers();
	});
</script>

<svelte:head>
	<title>Users - Admin - SceneForged</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-3">
			<Users class="h-6 w-6" />
			<h1 class="text-2xl font-bold">Users</h1>
		</div>
		<div class="flex items-center gap-2">
			<Button variant="outline" size="sm" onclick={loadUsers} disabled={loading}>
				<RefreshCw class="mr-2 h-4 w-4 {loading ? 'animate-spin' : ''}" />
				Refresh
			</Button>
			<Button size="sm" onclick={() => (showCreateDialog = true)}>
				<Plus class="mr-2 h-4 w-4" />
				Add User
			</Button>
		</div>
	</div>

	{#if error}
		<div class="rounded-md bg-destructive/10 p-4 text-destructive">
			{error}
		</div>
	{/if}

	<!-- Users Table -->
	<Card>
		<CardHeader>
			<CardTitle>All Users</CardTitle>
		</CardHeader>
		<CardContent>
			{#if loading && users.length === 0}
				<div class="flex items-center justify-center py-12">
					<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
				</div>
			{:else if users.length === 0}
				<div class="py-12 text-center">
					<Users class="mx-auto mb-4 h-12 w-12 text-muted-foreground/30" />
					<p class="text-muted-foreground">No users found</p>
				</div>
			{:else}
				<div class="rounded-md border">
					<table class="w-full">
						<thead>
							<tr class="border-b bg-muted/50">
								<th class="px-4 py-3 text-left text-sm font-medium">Username</th>
								<th class="px-4 py-3 text-left text-sm font-medium">Role</th>
								<th class="px-4 py-3 text-left text-sm font-medium">Created</th>
								<th class="w-24 px-4 py-3 text-left text-sm font-medium">Actions</th>
							</tr>
						</thead>
						<tbody>
							{#each users as user (user.id)}
								<tr class="border-b transition-colors hover:bg-muted/50">
									<td class="px-4 py-3 text-sm font-medium">
										{user.username}
										{#if isSelf(user)}
											<span class="ml-1 text-xs text-muted-foreground">(you)</span>
										{/if}
									</td>
									<td class="px-4 py-3">
										<Badge
											variant={user.role === 'admin' ? 'default' : 'secondary'}
										>
											{#if user.role === 'admin'}
												<ShieldCheck class="mr-1 h-3 w-3" />
											{:else}
												<Shield class="mr-1 h-3 w-3" />
											{/if}
											{user.role}
										</Badge>
									</td>
									<td class="px-4 py-3 text-sm text-muted-foreground">
										{new Date(user.created_at).toLocaleDateString()}
									</td>
									<td class="px-4 py-3">
										<div class="flex items-center gap-1">
											<Button
												variant="ghost"
												size="icon"
												onclick={() => openEdit(user)}
												title="Edit"
											>
												<Pencil class="h-4 w-4" />
											</Button>
											<Button
												variant="ghost"
												size="icon"
												onclick={() => openDelete(user)}
												title="Delete"
												disabled={isSelf(user)}
											>
												<Trash2 class="h-4 w-4" />
											</Button>
										</div>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</CardContent>
	</Card>
</div>

<!-- Create User Dialog -->
<Dialog.Root bind:open={showCreateDialog}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>Add User</Dialog.Title>
			<Dialog.Description>Create a new user account</Dialog.Description>
		</Dialog.Header>
		<div class="space-y-4">
			<div>
				<label for="new-username" class="mb-1 block text-sm font-medium">Username</label>
				<Input id="new-username" bind:value={newUsername} placeholder="Enter username" />
			</div>
			<div>
				<label for="new-password" class="mb-1 block text-sm font-medium">Password</label>
				<Input
					id="new-password"
					type="password"
					bind:value={newPassword}
					placeholder="Enter password"
				/>
			</div>
			<div>
				<label class="mb-2 block text-sm font-medium">Role</label>
				<div class="flex gap-4">
					<label class="flex items-center gap-2">
						<input type="radio" bind:group={newRole} value="user" />
						<span class="text-sm">User</span>
					</label>
					<label class="flex items-center gap-2">
						<input type="radio" bind:group={newRole} value="admin" />
						<span class="text-sm">Admin</span>
					</label>
				</div>
			</div>
			{#if createError}
				<p class="text-sm text-destructive">{createError}</p>
			{/if}
			<div class="flex justify-end gap-2">
				<Button variant="outline" onclick={() => (showCreateDialog = false)}>Cancel</Button>
				<Button
					onclick={handleCreate}
					disabled={creating || !newUsername.trim() || !newPassword.trim()}
				>
					{#if creating}
						<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					{/if}
					Create
				</Button>
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>

<!-- Edit User Dialog -->
<Dialog.Root bind:open={showEditDialog}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title>Edit User</Dialog.Title>
			<Dialog.Description>
				{editingUser ? `Editing ${editingUser.username}` : ''}
			</Dialog.Description>
		</Dialog.Header>
		{#if editingUser}
			<div class="space-y-4">
				<div>
					<label class="mb-2 block text-sm font-medium">Role</label>
					<div class="flex gap-4">
						<label class="flex items-center gap-2">
							<input type="radio" bind:group={editRole} value="user" />
							<span class="text-sm">User</span>
						</label>
						<label class="flex items-center gap-2">
							<input type="radio" bind:group={editRole} value="admin" />
							<span class="text-sm">Admin</span>
						</label>
					</div>
				</div>
				<div>
					<label for="edit-password" class="mb-1 block text-sm font-medium"
						>New Password (optional)</label
					>
					<Input
						id="edit-password"
						type="password"
						bind:value={editPassword}
						placeholder="Leave empty to keep current"
					/>
				</div>
				{#if editError}
					<p class="text-sm text-destructive">{editError}</p>
				{/if}
				<div class="flex justify-end gap-2">
					<Button variant="outline" onclick={() => (showEditDialog = false)}>Cancel</Button>
					<Button onclick={handleEdit} disabled={saving}>
						{#if saving}
							<Loader2 class="mr-2 h-4 w-4 animate-spin" />
						{/if}
						Save
					</Button>
				</div>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>

<!-- Delete Confirmation -->
<AlertDialog.Root bind:open={showDeleteDialog}>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>Delete User</AlertDialog.Title>
			<AlertDialog.Description>
				Are you sure you want to delete "{deletingUser?.username}"? This action cannot be
				undone.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel onclick={() => (showDeleteDialog = false)}>Cancel</AlertDialog.Cancel>
			<AlertDialog.Action onclick={handleDelete} disabled={deleting}>
				{#if deleting}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
				{/if}
				Delete
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
