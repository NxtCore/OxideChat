<template>
	<SettingsPage :title="t('title')" :description="t('description')">
		<template #actions>
			<ShadButton v-if="canEdit" size="sm" class="gap-2" data-testid="role-create" @click="openEditor(null)">
				<Plus class="h-4 w-4" />
				<span>{{ t('create') }}</span>
			</ShadButton>
		</template>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>
		<div v-else-if="roles.length === 0" class="py-12 text-center text-sm text-muted-foreground">{{ t('empty') }}</div>
		<div v-else class="divide-y divide-border rounded-lg border border-border bg-card">
			<div v-for="role in roles" :key="role.id" class="flex items-center gap-4 px-4 py-3" data-testid="role-row" :data-role-name="role.name">
				<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-primary/10">
					<Lock v-if="role.is_builtin" class="h-4 w-4 text-primary" />
					<ShieldCheck v-else class="h-4 w-4 text-primary" />
				</div>
				<div class="min-w-0 flex-1">
					<div class="flex flex-wrap items-center gap-2">
						<span class="font-medium text-foreground">{{ role.name }}</span>
						<span v-if="role.is_builtin" class="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">{{ t('builtin') }}</span>
					</div>
					<p class="text-xs text-muted-foreground">
						{{ store.getTranslation('roles.permission_count', {count: role.permissions.length}) }} ·
						{{ store.getTranslation('roles.user_count', {count: role.user_count}) }}
					</p>
				</div>
				<ShadButton variant="ghost" size="icon" class="h-8 w-8" data-testid="role-edit" :aria-label="isEditable(role) ? t('edit') : t('view')" @click="openEditor(role)">
					<Pencil v-if="isEditable(role)" class="h-4 w-4" />
					<Eye v-else class="h-4 w-4" />
				</ShadButton>
				<ShadButton v-if="isEditable(role)" variant="ghost" size="icon" class="h-8 w-8 text-destructive" data-testid="role-delete" :aria-label="t('delete')" @click="deleting = role">
					<Trash2 class="h-4 w-4" />
				</ShadButton>
			</div>
		</div>

		<ShadDialog :open="editorOpen" @update:open="open => !open && closeEditor()">
			<ShadDialogContent class="flex max-h-[85vh] flex-col sm:max-w-[640px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ editing ? (editorReadonly ? t('view') : t('edit')) : t('create') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ editorHint }}</ShadDialogDescription>
				</ShadDialogHeader>
				<div class="space-y-2">
					<ShadLabel for="role-name">{{ t('name') }}</ShadLabel>
					<ShadInput id="role-name" v-model="form.name" data-testid="role-name" maxlength="50" :disabled="editorReadonly" />
				</div>
				<div class="min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
					<ShadLabel>{{ t('permissions') }}</ShadLabel>
					<div v-for="group in groups" :key="group.id" class="space-y-1" :data-testid="`role-group-${group.id}`">
						<p class="text-xs font-medium tracking-wide text-muted-foreground uppercase">{{ store.getTranslation(`roles.group.${group.id}`) }}</p>
						<label v-for="permission in group.permissions" :key="permission" class="flex items-start gap-3 rounded-md px-2 py-1.5 hover:bg-accent/50">
							<ShadCheckbox
								class="mt-0.5"
								:data-testid="`role-permission-${permission}`"
								:model-value="form.permissions.includes(permission) || impliedBy(permission)"
								:disabled="editorReadonly || impliedBy(permission) || !store.hasPermission(permission)"
								@update:model-value="checked => toggle(permission, Boolean(checked))"
							/>
							<span class="min-w-0">
								<span class="block text-sm text-foreground">{{ store.getTranslation(`roles.permission.${permission}`) }}</span>
								<span class="block font-mono text-xs text-muted-foreground" data-allow-key>{{ permission }}</span>
							</span>
						</label>
					</div>
				</div>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="closeEditor">{{ store.getTranslation(editorReadonly ? 'common.close' : 'common.cancel') }}</ShadButton>
					<ShadButton v-if="!editorReadonly" data-testid="role-save" :disabled="saving || !form.name.trim()" @click="save">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin" />
						{{ store.getTranslation('common.save') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>

		<ShadDialog :open="!!deleting" @update:open="open => !open && (deleting = null)">
			<ShadDialogContent class="sm:max-w-[480px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ t('delete') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('roles.delete_confirm', {name: deleting?.name ?? '', count: deleting?.user_count ?? 0}) }}</ShadDialogDescription>
				</ShadDialogHeader>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="deleting = null">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton variant="destructive" data-testid="role-delete-confirm" :disabled="saving" @click="remove">{{ store.getTranslation('common.delete') }}</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</SettingsPage>
</template>

<script setup lang="ts">
import {Eye, Loader2, Lock, Pencil, Plus, ShieldCheck, Trash2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {ADMIN_ROLES_EDIT} from '~/types/generated/permissions';
import type {PermissionResponse} from '~/types/generated/PermissionResponse';
import type {RoleResponse} from '~/types/generated/RoleResponse';
import type {SaveRoleRequest} from '~/types/generated/SaveRoleRequest';

const GROUP_ORDER = ['wildcard', 'settings'];

const store = useMainStore();
const {$customFetch} = useNuxtApp();

const t = (key: string) => store.getTranslation(`roles.${key}`);

const roles = ref<RoleResponse[]>([]);
const permissions = ref<PermissionResponse[]>([]);
const loading = ref(true);
const saving = ref(false);
const editorOpen = ref(false);
const editing = ref<RoleResponse | null>(null);
const deleting = ref<RoleResponse | null>(null);
const form = reactive<SaveRoleRequest>({name: '', permissions: []});

const canEdit = computed(() => store.hasPermission(ADMIN_ROLES_EDIT));
const editorReadonly = computed(() => !!editing.value && !isEditable(editing.value));
const editorHint = computed(() => {
	if (editing.value?.is_builtin) return t('builtin_hint');
	return t('not_held_hint');
});

const groups = computed(() => {
	const byGroup = new Map<string, string[]>();
	for (const permission of permissions.value) {
		byGroup.set(permission.group, [...(byGroup.get(permission.group) ?? []), permission.name]);
	}
	const rank = (id: string) => (GROUP_ORDER.includes(id) ? GROUP_ORDER.indexOf(id) : GROUP_ORDER.length);
	return [...byGroup.entries()].map(([id, names]) => ({id, permissions: names})).sort((a, b) => rank(a.id) - rank(b.id) || a.id.localeCompare(b.id));
});

function isEditable(role: RoleResponse): boolean {
	return canEdit.value && !role.is_builtin && role.permissions.every(permission => store.hasPermission(permission));
}

function impliedBy(permission: string): boolean {
	return form.permissions.some(selected => selected !== permission && selected.endsWith('.*') && permission.startsWith(selected.slice(0, -1)));
}

function toggle(permission: string, checked: boolean) {
	form.permissions = checked ? [...form.permissions, permission] : form.permissions.filter(selected => selected !== permission);
}

function openEditor(role: RoleResponse | null) {
	editing.value = role;
	form.name = role?.name ?? '';
	form.permissions = [...(role?.permissions ?? [])];
	editorOpen.value = true;
}

function closeEditor() {
	editorOpen.value = false;
	editing.value = null;
}

async function load() {
	loading.value = true;
	try {
		[roles.value, permissions.value] = await Promise.all([
			$customFetch<RoleResponse[]>('/api/v1/admin/roles'),
			$customFetch<PermissionResponse[]>('/api/v1/admin/permissions'),
		]);
	} catch (e: any) {
		store.toast(t('load_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		loading.value = false;
	}
}

async function save() {
	saving.value = true;
	try {
		const body: SaveRoleRequest = {name: form.name.trim(), permissions: form.permissions};
		const role = editing.value
			? await $customFetch<RoleResponse>(`/api/v1/admin/roles/${editing.value.id}`, {method: 'PUT', body})
			: await $customFetch<RoleResponse>('/api/v1/admin/roles', {method: 'POST', body});
		roles.value = editing.value ? roles.value.map(row => (row.id === role.id ? role : row)) : [...roles.value, role];
		store.toast(t('saved'), {type: 'success'});
		closeEditor();
		await store.refreshRoles();
	} catch (e: any) {
		store.toast(t('save_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}

async function remove() {
	const role = deleting.value;
	if (!role) return;
	saving.value = true;
	try {
		await $customFetch(`/api/v1/admin/roles/${role.id}`, {method: 'DELETE'});
		roles.value = roles.value.filter(row => row.id !== role.id);
		store.toast(t('deleted'), {type: 'success'});
		deleting.value = null;
		await store.refreshRoles();
	} catch (e: any) {
		store.toast(t('delete_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}

onMounted(load);
</script>
