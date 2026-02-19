<template>
	<div class="max-w-4xl lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between mb-6">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tabs.admin_users') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.admin_users.description') }}</p>
			</div>
			<ShadButton v-if="store.hasPermission('admin.users.edit')" variant="default" size="sm" class="gap-2" @click="openCreateDialog">
				<Plus class="h-4 w-4" />
				<span>{{ store.getTranslation('settings.admin_users.create_user') }}</span>
			</ShadButton>
		</div>

		<div class="flex flex-row gap-2 mb-4">
			<Input
				v-model="searchRaw"
				type="text"
				:placeholder="store.getTranslation('settings.admin_users.search_placeholder')"
				class="flex-1"
			/>
			<Select v-model="roleFilter">
				<SelectTrigger class="w-36">
					<SelectValue :placeholder="store.getTranslation('settings.admin_users.filter_role')" />
				</SelectTrigger>
				<SelectContent>
					<SelectItem value="all">{{ store.getTranslation('settings.admin_users.role_all') }}</SelectItem>
					<SelectItem value="admin">{{ store.getTranslation('settings.admin_users.role_admin') }}</SelectItem>
					<SelectItem value="user">{{ store.getTranslation('settings.admin_users.role_user') }}</SelectItem>
				</SelectContent>
			</Select>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="users.length === 0" class="flex items-center justify-center py-12 text-muted-foreground">
			<div class="text-center">
				<Users class="h-12 w-12 mx-auto mb-4 opacity-50" />
				<p>{{ store.getTranslation('settings.admin_users.no_users') }}</p>
			</div>
		</div>

		<div v-else class="space-y-2">
			<div
				v-for="u in users"
				:key="u.id"
				class="rounded-lg border border-border bg-card px-4 py-3 flex items-center gap-4 transition-all hover:border-border/80"
			>
				<div class="flex-1 min-w-0">
					<div class="flex items-center gap-2 flex-wrap">
						<span class="font-medium text-foreground">{{ u.username }}</span>
						<span
							v-for="role in u.roles"
							:key="role"
							class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium"
							:class="role === 'admin' ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'"
						>{{ role }}</span>
					</div>
					<div class="flex items-center gap-3 mt-0.5 text-sm text-muted-foreground">
						<span>{{ u.email }}</span>
						<span class="text-border">·</span>
						<span>{{ u.auth_method }}</span>
						<span class="text-border">·</span>
						<span>{{ formatDate(u.created_at) }}</span>
					</div>
				</div>

				<div class="flex items-center gap-2 shrink-0">
					<ShadButton
						variant="outline"
						size="sm"
						:disabled="u.id === store.auth.user?.id"
						:title="u.id === store.auth.user?.id ? store.getTranslation('settings.admin_users.cannot_modify_self') : ''"
						@click="openEditDialog(u)"
					>
						<Pencil class="h-4 w-4" />
					</ShadButton>
					<ShadButton
						v-if="store.hasPermission('admin.users.edit')"
						variant="outline"
						size="sm"
						class="text-destructive hover:text-destructive"
						:disabled="u.id === store.auth.user?.id"
						:title="u.id === store.auth.user?.id ? store.getTranslation('settings.admin_users.cannot_modify_self') : ''"
						@click="openDeleteDialog(u)"
					>
						<Trash2 class="h-4 w-4" />
					</ShadButton>
				</div>
			</div>
		</div>

		<div v-if="total > 0" class="flex items-center justify-between mt-4 text-sm text-muted-foreground">
			<span>{{ paginationLabel }}</span>
			<div class="flex items-center gap-1">
				<ShadButton variant="outline" size="sm" :disabled="page <= 1" @click="page--">
					<ChevronLeft class="h-4 w-4" />
				</ShadButton>
				<ShadButton variant="outline" size="sm" :disabled="page >= totalPages" @click="page++">
					<ChevronRight class="h-4 w-4" />
				</ShadButton>
			</div>
		</div>

		<Dialog v-model:open="createOpen">
			<DialogContent class="sm:max-w-[480px]">
				<DialogHeader>
					<DialogTitle>{{ store.getTranslation('settings.admin_users.create_user') }}</DialogTitle>
					<DialogDescription>{{ store.getTranslation('settings.admin_users.create_user_description') }}</DialogDescription>
				</DialogHeader>
				<div class="space-y-4 py-2">
					<div class="space-y-2">
						<Label for="create-email">{{ store.getTranslation('settings.admin_users.field_email') }}</Label>
						<Input id="create-email" v-model="createForm.email" type="email" />
					</div>
					<div class="space-y-2">
						<Label for="create-username">{{ store.getTranslation('settings.admin_users.field_username') }}</Label>
						<Input id="create-username" v-model="createForm.username" type="text" />
					</div>
					<div class="space-y-2">
						<Label for="create-password">{{ store.getTranslation('settings.admin_users.field_password') }}</Label>
						<Input id="create-password" v-model="createForm.password" type="password" />
					</div>
					<div class="space-y-2">
						<Label>{{ store.getTranslation('settings.admin_users.field_roles') }}</Label>
						<div class="flex gap-3">
							<label v-for="role in availableRoles" :key="role" class="flex items-center gap-2 cursor-pointer">
								<input type="checkbox" :value="role" v-model="createForm.roles" class="accent-primary" />
								<span class="text-sm text-foreground">{{ role }}</span>
							</label>
						</div>
					</div>
				</div>
				<DialogFooter>
					<ShadButton variant="outline" @click="createOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton @click="submitCreate" :disabled="saving">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
						{{ store.getTranslation('common.create') }}
					</ShadButton>
				</DialogFooter>
			</DialogContent>
		</Dialog>

		<Dialog v-model:open="editOpen">
			<DialogContent class="sm:max-w-[480px]">
				<DialogHeader>
					<DialogTitle>{{ store.getTranslation('settings.admin_users.edit_user') }}</DialogTitle>
					<DialogDescription>{{ selectedUser?.username }}</DialogDescription>
				</DialogHeader>
				<div class="space-y-4 py-2">
					<div class="space-y-2">
						<Label for="edit-email">{{ store.getTranslation('settings.admin_users.field_email') }}</Label>
						<Input id="edit-email" v-model="editForm.email" type="email" />
					</div>
					<div class="space-y-2">
						<Label for="edit-username">{{ store.getTranslation('settings.admin_users.field_username') }}</Label>
						<Input id="edit-username" v-model="editForm.username" type="text" />
					</div>
					<div class="space-y-2">
						<Label>{{ store.getTranslation('settings.admin_users.field_roles') }}</Label>
						<div class="flex gap-3">
							<label v-for="role in availableRoles" :key="role" class="flex items-center gap-2 cursor-pointer">
								<input type="checkbox" :value="role" v-model="editForm.roles" class="accent-primary" />
								<span class="text-sm text-foreground">{{ role }}</span>
							</label>
						</div>
					</div>
					<div v-if="selectedUser?.auth_method === 'local'" class="space-y-2 border-t border-border pt-4">
						<Label for="edit-password">{{ store.getTranslation('settings.admin_users.reset_password') }}</Label>
						<Input id="edit-password" v-model="editForm.newPassword" type="password" :placeholder="store.getTranslation('settings.admin_users.reset_password_placeholder')" />
					</div>
				</div>
				<DialogFooter>
					<ShadButton variant="outline" @click="editOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton @click="submitEdit" :disabled="saving">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
						{{ store.getTranslation('common.save') }}
					</ShadButton>
				</DialogFooter>
			</DialogContent>
		</Dialog>

		<Dialog v-model:open="deleteOpen">
			<DialogContent class="sm:max-w-[420px]">
				<DialogHeader>
					<DialogTitle>{{ store.getTranslation('settings.admin_users.delete_user') }}</DialogTitle>
					<DialogDescription>
						{{ store.getTranslation('settings.admin_users.delete_confirm').replace('{username}', selectedUser?.username ?? '') }}
					</DialogDescription>
				</DialogHeader>
				<DialogFooter>
					<ShadButton variant="outline" @click="deleteOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton variant="destructive" @click="submitDelete" :disabled="saving">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
						{{ store.getTranslation('common.delete') }}
					</ShadButton>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	</div>
</template>

<script setup lang="ts">
import {ref, reactive, computed, watch, onMounted} from 'vue';
import {Users, Plus, Pencil, Trash2, Loader2, ChevronLeft, ChevronRight} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle} from '@/components/ui/dialog';
import {Input} from '@/components/ui/input';
import {Label} from '@/components/ui/label';
import {Select, SelectContent, SelectItem, SelectTrigger, SelectValue} from '@/components/ui/select';

const {$customFetch} = useNuxtApp();
const store = useMainStore();

interface UserResponse {
	id: string;
	email: string;
	username: string;
	auth_method: string;
	roles: string[];
	permissions: string[];
	created_at: string;
}

interface PaginatedUsersResponse {
	users: UserResponse[];
	total: number;
	page: number;
	per_page: number;
}

const availableRoles = ['admin', 'user'];

const users = ref<UserResponse[]>([]);
const total = ref(0);
const page = ref(1);
const perPage = ref(20);
const searchRaw = ref('');
const search = ref('');
const roleFilter = ref('all');
const loading = ref(false);
const saving = ref(false);

const createOpen = ref(false);
const editOpen = ref(false);
const deleteOpen = ref(false);
const selectedUser = ref<UserResponse | null>(null);

const createForm = reactive({email: '', username: '', password: '', roles: [] as string[]});
const editForm = reactive({email: '', username: '', roles: [] as string[], newPassword: ''});

const totalPages = computed(() => Math.ceil(total.value / perPage.value));

const paginationLabel = computed(() => {
	const start = (page.value - 1) * perPage.value + 1;
	const end = Math.min(page.value * perPage.value, total.value);
	return `${start}–${end} / ${total.value}`;
});

let searchTimer: ReturnType<typeof setTimeout> | null = null;
watch(searchRaw, val => {
	if (searchTimer) clearTimeout(searchTimer);
	searchTimer = setTimeout(() => {
		search.value = val;
		page.value = 1;
	}, 300);
});

watch([roleFilter, page], () => {
	loadUsers();
});

watch(search, () => {
	loadUsers();
});

async function loadUsers() {
	loading.value = true;
	try {
		const params: Record<string, string | number> = {page: page.value, per_page: perPage.value};
		if (search.value) params.search = search.value;
		if (roleFilter.value !== 'all') params.role = roleFilter.value;

		const result = await $customFetch<PaginatedUsersResponse>('/api/v1/admin/users', {params});
		users.value = result.users ?? [];
		total.value = result.total ?? 0;
	} catch (e: any) {
		store.toast(store.getTranslation('settings.admin_users.load_error'), {type: 'error', description: e?.message ?? ''});
	} finally {
		loading.value = false;
	}
}

function formatDate(iso: string) {
	return new Date(iso).toLocaleDateString(undefined, {year: 'numeric', month: 'short', day: 'numeric'});
}

function openCreateDialog() {
	createForm.email = '';
	createForm.username = '';
	createForm.password = '';
	createForm.roles = ['user'];
	createOpen.value = true;
}

function openEditDialog(u: UserResponse) {
	selectedUser.value = u;
	editForm.email = u.email;
	editForm.username = u.username;
	editForm.roles = [...u.roles];
	editForm.newPassword = '';
	editOpen.value = true;
}

function openDeleteDialog(u: UserResponse) {
	selectedUser.value = u;
	deleteOpen.value = true;
}

async function submitCreate() {
	saving.value = true;
	try {
		await $customFetch('/api/v1/admin/users', {
			method: 'POST',
			body: {email: createForm.email, username: createForm.username, password: createForm.password, roles: createForm.roles},
		});
		store.toast(store.getTranslation('settings.admin_users.create_success'), {type: 'success'});
		createOpen.value = false;
		await loadUsers();
	} catch (e: any) {
		store.toast(store.getTranslation('settings.admin_users.create_error'), {type: 'error', description: e?.message ?? ''});
	} finally {
		saving.value = false;
	}
}

async function submitEdit() {
	if (!selectedUser.value) return;
	saving.value = true;
	try {
		await $customFetch(`/api/v1/admin/users/${selectedUser.value.id}`, {
			method: 'PUT',
			body: {email: editForm.email, username: editForm.username},
		});

		await $customFetch(`/api/v1/admin/users/${selectedUser.value.id}/roles`, {
			method: 'PUT',
			body: {roles: editForm.roles},
		});

		if (editForm.newPassword) {
			await $customFetch(`/api/v1/admin/users/${selectedUser.value.id}/password`, {
				method: 'PUT',
				body: {password: editForm.newPassword},
			});
		}

		store.toast(store.getTranslation('settings.admin_users.edit_success'), {type: 'success'});
		editOpen.value = false;
		await loadUsers();
	} catch (e: any) {
		store.toast(store.getTranslation('settings.admin_users.edit_error'), {type: 'error', description: e?.message ?? ''});
	} finally {
		saving.value = false;
	}
}

async function submitDelete() {
	if (!selectedUser.value) return;
	saving.value = true;
	try {
		await $customFetch(`/api/v1/admin/users/${selectedUser.value.id}`, {method: 'DELETE'});
		store.toast(store.getTranslation('settings.admin_users.delete_success'), {type: 'success'});
		deleteOpen.value = false;
		await loadUsers();
	} catch (e: any) {
		store.toast(store.getTranslation('settings.admin_users.delete_error'), {type: 'error', description: e?.message ?? ''});
	} finally {
		saving.value = false;
	}
}

onMounted(() => {
	loadUsers();
});
</script>
