<template>
	<div class="w-full lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between mb-6">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tabs.admin_users') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.admin_users.description') }}</p>
			</div>
			<ShadButton v-if="store.hasPermission(ADMIN_USERS_EDIT)" variant="default" size="sm" class="gap-2" @click="openCreateDialog">
				<Plus class="h-4 w-4" />
				<span>{{ store.getTranslation('settings.admin_users.create_user') }}</span>
			</ShadButton>
		</div>

		<div class="flex flex-row gap-2 mb-4">
			<ShadInput v-model="searchRaw" type="text" :placeholder="store.getTranslation('settings.admin_users.search_placeholder')" class="flex-1" />
			<ShadSelect v-model="roleFilter" clearable>
				<ShadSelectTrigger class="w-36">
					<ShadSelectValue :placeholder="store.getTranslation('settings.admin_users.filter_role')" />
				</ShadSelectTrigger>
				<ShadSelectContent>
					<ShadSelectItem v-for="role in availableRoles" :key="role.id" :value="role.name">{{ role.name }}</ShadSelectItem>
				</ShadSelectContent>
			</ShadSelect>
			<ShadSelect v-model="teamFilter" clearable>
				<ShadSelectTrigger class="w-44">
					<ShadSelectValue :placeholder="store.getTranslation('settings.admin_users.filter_team')" />
				</ShadSelectTrigger>
				<ShadSelectContent>
					<ShadSelectItem v-for="team in availableTeams" :key="team.id" :value="team.id">{{ team.name }}</ShadSelectItem>
				</ShadSelectContent>
			</ShadSelect>
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
				v-for="user in users"
				:key="user.id"
				class="rounded-lg border border-border bg-card px-4 py-3 flex items-center gap-4 transition-all hover:border-border/80"
			>
				<div class="flex-1 min-w-0">
					<div class="flex items-center gap-2 flex-wrap">
						<span class="font-medium text-foreground">{{ user.username }}</span>
						<span
							v-for="role in user.roles"
							:key="role"
							class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium"
							:class="role === 'admin' ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'"
						>
							{{ role }}
						</span>
						<span
							v-for="team in user.teams"
							:key="team.id"
							class="inline-flex items-center rounded-full bg-accent px-2 py-0.5 text-xs font-medium text-accent-foreground"
						>
							{{ team.name }}
						</span>
					</div>
					<div class="flex items-center gap-3 mt-0.5 text-sm text-muted-foreground">
						<span>{{ user.email }}</span>
						<span class="text-border">·</span>
						<span>{{ user.auth_method }}</span>
						<span class="text-border">·</span>
						<span>{{ formatDate(user.created_at) }}</span>
					</div>
				</div>

				<div class="flex items-center gap-2 shrink-0">
					<ShadButton
						variant="outline"
						size="sm"
						:title="store.getTranslation('settings.admin_users.view_analytics')"
						@click="router.push(`/settings/analytics?user_id=${user.id}`)"
					>
						<BarChart2 class="h-4 w-4" />
					</ShadButton>
					<ShadButton
						variant="outline"
						size="sm"
						:disabled="user.id === store.auth.user?.id || !store.hasPermission(ADMIN_USERS_EDIT)"
						:title="user.id === store.auth.user?.id ? store.getTranslation('settings.admin_users.cannot_modify_self') : ''"
						@click="openEditDialog(user)"
					>
						<Pencil class="h-4 w-4" />
					</ShadButton>
					<ShadButton
						variant="outline"
						size="sm"
						class="text-destructive hover:text-destructive"
						:disabled="user.id === store.auth.user?.id || !store.hasPermission(ADMIN_USERS_EDIT)"
						:title="user.id === store.auth.user?.id ? store.getTranslation('settings.admin_users.cannot_modify_self') : ''"
						@click="openDeleteDialog(user)"
					>
						<Trash2 class="h-4 w-4" />
					</ShadButton>
				</div>
			</div>
		</div>

		<AppPagination v-model="page" :has-more="hasMore" />

		<ShadDialog v-model:open="createOpen">
			<ShadDialogContent class="sm:max-w-[480px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('settings.admin_users.create_user') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('settings.admin_users.create_user_description') }}</ShadDialogDescription>
				</ShadDialogHeader>
				<div class="space-y-4 py-2">
					<div class="space-y-2">
						<ShadLabel for="create-email">{{ store.getTranslation('settings.admin_users.field_email') }}</ShadLabel>
						<ShadInput id="create-email" v-model="createForm.email" type="email" />
					</div>
					<div class="space-y-2">
						<ShadLabel for="create-username">{{ store.getTranslation('settings.admin_users.field_username') }}</ShadLabel>
						<ShadInput id="create-username" v-model="createForm.username" type="text" />
					</div>
					<div class="space-y-2">
						<ShadLabel for="create-password">{{ store.getTranslation('settings.admin_users.field_password') }}</ShadLabel>
						<ShadInput id="create-password" v-model="createForm.password" type="password" />
					</div>
					<div class="space-y-2">
						<ShadLabel>{{ store.getTranslation('settings.admin_users.field_roles') }}</ShadLabel>
						<ShadSelect v-model="createForm.roles" multiple clearable>
							<ShadSelectTrigger class="w-full">
								<ShadSelectValue :placeholder="store.getTranslation('settings.admin_users.select_roles')" />
							</ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="role in availableRoles" :key="role.id" :value="role.name">{{ role.name }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div class="space-y-2">
						<ShadLabel>{{ store.getTranslation('settings.admin_users.field_teams') }}</ShadLabel>
						<ShadSelect v-model="createForm.team_ids" multiple>
							<ShadSelectTrigger class="w-full">
								<ShadSelectValue :placeholder="store.getTranslation('settings.admin_users.select_teams')" />
							</ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="team in availableTeams" :key="team.id" :value="team.id" :disabled="team.is_default">{{ team.name }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
				</div>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="createOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton @click="submitCreate" :disabled="saving">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
						{{ store.getTranslation('common.create') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>

		<ShadDialog v-model:open="editOpen">
			<ShadDialogContent class="sm:max-w-[480px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('settings.admin_users.edit_user') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ selectedUser?.username }}</ShadDialogDescription>
				</ShadDialogHeader>
				<div class="space-y-4 py-2">
					<div class="space-y-2">
						<ShadLabel for="edit-email">{{ store.getTranslation('settings.admin_users.field_email') }}</ShadLabel>
						<ShadInput id="edit-email" v-model="editForm.email" type="email" />
					</div>
					<div class="space-y-2">
						<ShadLabel for="edit-username">{{ store.getTranslation('settings.admin_users.field_username') }}</ShadLabel>
						<ShadInput id="edit-username" v-model="editForm.username" type="text" />
					</div>
					<div class="space-y-2">
						<ShadLabel>{{ store.getTranslation('settings.admin_users.field_roles') }}</ShadLabel>
						<ShadSelect v-model="editForm.roles" multiple clearable>
							<ShadSelectTrigger class="w-full">
								<ShadSelectValue :placeholder="store.getTranslation('settings.admin_users.select_roles')" />
							</ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="role in availableRoles" :key="role.id" :value="role.name">{{ role.name }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div class="space-y-2">
						<ShadLabel>{{ store.getTranslation('settings.admin_users.field_teams') }}</ShadLabel>
						<ShadSelect v-model="editForm.team_ids" multiple>
							<ShadSelectTrigger class="w-full">
								<ShadSelectValue :placeholder="store.getTranslation('settings.admin_users.select_teams')" />
							</ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="team in availableTeams" :key="team.id" :value="team.id" :disabled="team.is_default">{{ team.name }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div v-if="selectedUser?.auth_method === 'local'" class="space-y-2 border-t border-border pt-4">
						<ShadLabel for="edit-password">{{ store.getTranslation('settings.admin_users.reset_password') }}</ShadLabel>
						<ShadInput
							id="edit-password"
							v-model="editForm.newPassword"
							type="password"
							:placeholder="store.getTranslation('settings.admin_users.reset_password_placeholder')"
						/>
					</div>
				</div>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="editOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton @click="submitEdit" :disabled="saving">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
						{{ store.getTranslation('common.save') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>

		<ShadDialog v-model:open="deleteOpen">
			<ShadDialogContent class="sm:max-w-[420px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('settings.admin_users.delete_user') }}</ShadDialogTitle>
					<ShadDialogDescription>
						{{ store.getTranslation('settings.admin_users.delete_confirm').replace('{username}', selectedUser?.username ?? '') }}
					</ShadDialogDescription>
				</ShadDialogHeader>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="deleteOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton variant="destructive" @click="submitDelete" :disabled="saving">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
						{{ store.getTranslation('common.delete') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {ADMIN_USERS_EDIT} from '~/types/generated/permissions';
import {ref, reactive, computed, watch, onMounted} from 'vue';
import {Users, Plus, Pencil, Trash2, Loader2, BarChart2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import type {TeamList, UserListResponse} from '~/types/chat';

const store = useMainStore();
const router = useRouter();

const users = ref<UserListResponse[]>([]);
const hasMore = ref(false);
const page = ref(1);
const searchRaw = ref('');
const search = ref('');
const roleFilter = ref(null);
const teamFilter = ref(null);
const loading = ref(false);
const saving = ref(false);

const createOpen = ref(false);
const editOpen = ref(false);
const deleteOpen = ref(false);
const selectedUser = ref<UserListResponse | null>(null);
const teams = ref<TeamList[]>([]);

const defaultTeamIds = computed(() => teams.value.filter(t => t.is_default).map(t => t.id));
function ensureDefaultTeams(ids: string[]) {
	for (const id of defaultTeamIds.value) {
		if (!ids.includes(id)) ids.push(id);
	}
}

const createForm = reactive({email: '', username: '', password: '', roles: [] as string[], team_ids: [] as string[]});
const editForm = reactive({email: '', username: '', roles: [] as string[], team_ids: [] as string[], newPassword: ''});

watch(() => createForm.team_ids, () => ensureDefaultTeams(createForm.team_ids));
watch(() => editForm.team_ids, () => ensureDefaultTeams(editForm.team_ids));

const availableRoles = computed(() => store.roles);
const availableTeams = computed(() => teams.value);

let searchTimer: ReturnType<typeof setTimeout> | null = null;
watch(searchRaw, val => {
	if (searchTimer) clearTimeout(searchTimer);
	searchTimer = setTimeout(() => {
		search.value = val;
		page.value = 1;
	}, 300);
});

watch([roleFilter, teamFilter, page], () => {
	loadUsers();
});

watch(search, () => {
	loadUsers();
});

async function loadUsers() {
	loading.value = true;
	try {
		const params: Record<string, string | number> = {page: page.value};
		if (search.value) params.search = search.value;
		if (roleFilter.value) params.role = roleFilter.value;
		if (teamFilter.value) params.team_id = teamFilter.value;

		const result = await adminUsers(params);
		users.value = result.items ?? [];
		hasMore.value = result.has_more ?? false;
	} catch (e: any) {
		const errorMessage = e?.data?.errors?.[0]?.message || e?.message || '';
		store.toast(store.getTranslation('settings.admin_users.load_error'), {type: 'error', description: errorMessage});
	} finally {
		loading.value = false;
	}
}

async function loadTeams() {
	try {
		const result = await adminTeams({size: 100});
		teams.value = result.items ?? [];
	} catch (e: any) {
		const errorMessage = e?.data?.errors?.[0]?.message || e?.message || '';
		store.toast(store.getTranslation('settings.teams.load_error'), {type: 'error', description: errorMessage});
	}
}

function openCreateDialog() {
	createForm.email = '';
	createForm.username = '';
	createForm.password = '';
	createForm.roles = ['user'];
	createForm.team_ids = teams.value.filter(team => team.is_default).map(team => team.id);
	createOpen.value = true;
}

function openEditDialog(u: UserListResponse) {
	selectedUser.value = u;
	editForm.email = u.email;
	editForm.username = u.username;
	editForm.roles = [...u.roles];
	editForm.team_ids = u.teams.map(team => team.id);
	editForm.newPassword = '';
	editOpen.value = true;
}

function openDeleteDialog(u: UserListResponse) {
	selectedUser.value = u;
	deleteOpen.value = true;
}

async function submitCreate() {
	saving.value = true;
	try {
		await createAdminUser({email: createForm.email, username: createForm.username, password: createForm.password, roles: createForm.roles, team_ids: createForm.team_ids});
		store.toast(store.getTranslation('settings.admin_users.create_success'), {type: 'success'});
		createOpen.value = false;
		await loadUsers();
	} catch (e: any) {
		const errorMessage = e?.data?.errors?.[0]?.message || e?.message || '';
		store.toast(store.getTranslation('settings.admin_users.create_error'), {type: 'error', description: errorMessage});
	} finally {
		saving.value = false;
	}
}

async function submitEdit() {
	if (!selectedUser.value) return;
	saving.value = true;
	try {
		await updateAdminUser(selectedUser.value.id, {email: editForm.email, username: editForm.username});

		await updateAdminUser(selectedUser.value.id, {roles: editForm.roles}, '/roles');

		await updateAdminUser(selectedUser.value.id, {team_ids: editForm.team_ids}, '/teams');

		if (editForm.newPassword) {
			await updateAdminUser(selectedUser.value.id, {password: editForm.newPassword}, '/password');
		}

		store.toast(store.getTranslation('settings.admin_users.edit_success'), {type: 'success'});
		editOpen.value = false;
		await loadUsers();
	} catch (e: any) {
		const errorMessage = e?.data?.errors?.[0]?.message || e?.message || '';
		store.toast(store.getTranslation('settings.admin_users.edit_error'), {type: 'error', description: errorMessage});
	} finally {
		saving.value = false;
	}
}

async function submitDelete() {
	if (!selectedUser.value) return;
	saving.value = true;
	try {
		await deleteAdminUser(selectedUser.value.id);
		store.toast(store.getTranslation('settings.admin_users.delete_success'), {type: 'success'});
		deleteOpen.value = false;
		await loadUsers();
	} catch (e: any) {
		const errorMessage = e?.data?.errors?.[0]?.message || e?.message || '';
		store.toast(store.getTranslation('settings.admin_users.delete_error'), {type: 'error', description: errorMessage});
	} finally {
		saving.value = false;
	}
}

onMounted(async () => {
	await loadTeams();
	await loadUsers();
});
</script>
