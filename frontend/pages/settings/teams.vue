<template>
	<SettingsPage :title="store.getTranslation('settings.tabs.teams')" :description="store.getTranslation('settings.teams.description')" :form="settingsForm">
		<template #actions>
			<ShadButton
				v-if="canEdit && selectedTeam && !selectedTeam.is_default"
				variant="outline"
				size="sm"
				class="gap-2 text-destructive hover:text-destructive"
				:disabled="saving"
				@click="deleteOpen = true"
			>
				<Trash2 class="h-4 w-4" />
			</ShadButton>
			<ShadButton v-if="canEdit" variant="default" size="sm" class="gap-2" @click="openCreate">
				<Plus class="h-4 w-4" />
				<span>{{ store.getTranslation('settings.teams.create') }}</span>
			</ShadButton>
		</template>

		<div class="grid gap-4 lg:grid-cols-[19rem_1fr]">
			<div class="space-y-3">
				<div class="relative">
					<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
					<ShadInput v-model="search" :placeholder="store.getTranslation('settings.teams.search')" class="pl-8" />
				</div>

				<div v-if="loadingTeams" class="flex items-center justify-center py-10 text-muted-foreground">
					<Loader2 class="h-5 w-5 animate-spin" />
				</div>
				<div v-else class="space-y-2">
					<button
						v-for="team in teams"
						:key="team.id"
						type="button"
						class="w-full rounded-lg border px-3 py-2.5 text-left transition-colors"
						:class="selectedTeam?.id === team.id ? 'border-primary/60 bg-accent/40' : 'border-border bg-card hover:border-primary/40 hover:bg-accent/10'"
						@click="selectTeam(team.id)"
					>
						<div class="flex items-center justify-between gap-2">
							<span class="truncate font-medium text-foreground">{{ team.name }}</span>
							<span v-if="team.is_default" class="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
								{{ store.getTranslation('settings.teams.default') }}
							</span>
						</div>
						<div class="mt-1 flex items-center gap-3 text-xs text-muted-foreground">
							<span class="inline-flex items-center gap-1">
								<Users class="h-3 w-3" />
								{{ store.getTranslation('settings.teams.members_count', {count: team.member_count}) }}
							</span>
							<span v-if="team.allow_all_models" class="inline-flex items-center gap-1">
								<Bot class="h-3 w-3" />
								{{ store.getTranslation('settings.teams.allow_all_models') }}
							</span>
						</div>
					</button>
				</div>
			</div>

			<div v-if="selectedTeam" class="space-y-4">
				<div class="rounded-lg border border-border bg-card p-4">
					<div class="grid gap-4 md:grid-cols-2">
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.teams.name') }}</ShadLabel>
							<ShadInput v-model="form.name" :disabled="!canEdit || selectedTeam.is_default" />
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.teams.description_field') }}</ShadLabel>
							<ShadTextarea v-model="form.description" :disabled="!canEdit" rows="1" />
						</div>
					</div>
				</div>

				<ShadTabs v-model="activeTab" class="w-full">
					<ShadTabsList>
						<ShadTabsTrigger value="members">{{ store.getTranslation('settings.teams.members') }}</ShadTabsTrigger>
						<ShadTabsTrigger value="models">{{ store.getTranslation('settings.teams.models') }}</ShadTabsTrigger>
					</ShadTabsList>

					<!-- Members -->
					<ShadTabsContent value="members" class="mt-4 rounded-lg border border-border bg-card">
						<div class="flex items-center gap-2 border-b border-border p-3">
							<div class="relative flex-1">
								<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
								<ShadInput v-model="memberSearch" :placeholder="store.getTranslation('settings.teams.search_members')" class="pl-8 h-9" />
							</div>
							<span class="shrink-0 text-xs text-muted-foreground">{{ store.getTranslation('settings.teams.selected_count', {count: form.member_ids.length}) }}</span>
						</div>
						<div class="max-h-80 overflow-y-auto p-2">
							<p v-if="filteredUsers.length === 0" class="py-8 text-center text-sm text-muted-foreground">
								{{ store.getTranslation('settings.teams.no_members') }}
							</p>
							<label
								v-for="user in filteredUsers"
								:key="user.id"
								class="flex cursor-pointer items-center gap-3 rounded-md px-2 py-2 hover:bg-accent/40"
							>
								<ShadCheckbox
									:model-value="form.member_ids.includes(user.id)"
									:disabled="!canEdit || selectedTeam.is_default"
									@update:model-value="checked => toggleMember(user.id, Boolean(checked))"
								/>
								<span class="min-w-0 flex-1">
									<span class="block truncate text-sm text-foreground">{{ user.username }}</span>
									<span class="block truncate text-xs text-muted-foreground">{{ user.email }}</span>
								</span>
								<ShadSelect
									v-if="form.member_ids.includes(user.id)"
									:model-value="form.member_roles[user.id] ?? 'member'"
									:disabled="!canEdit || settingsForm.saving"
									@update:model-value="role => (form.member_roles[user.id] = role as TeamRole)"
								>
									<ShadSelectTrigger class="h-8 w-32" :data-testid="`member-role-${user.username}`" @click.stop><ShadSelectValue /></ShadSelectTrigger>
									<ShadSelectContent>
										<ShadSelectItem value="member">{{ store.getTranslation('settings.teams.role_member') }}</ShadSelectItem>
										<ShadSelectItem value="admin">{{ store.getTranslation('settings.teams.role_admin') }}</ShadSelectItem>
									</ShadSelectContent>
								</ShadSelect>
							</label>
						</div>
					</ShadTabsContent>

					<!-- Models -->
					<ShadTabsContent value="models" class="mt-4 rounded-lg border border-border bg-card">
						<div class="border-b border-border p-3 space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.teams.default_model') }}</ShadLabel>
							<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.teams.default_model_hint') }}</p>
							<DefaultModelPicker
								v-model="form.default_model_id"
								:disabled="!canEdit"
								:endpoint="adminModelsPath"
								:selected-model-endpoint="adminModelsPath"
								value-mode="uuid"
								:placeholder="store.getTranslation('settings.teams.use_global_default')"
							/>
						</div>
						<div class="flex items-center justify-between gap-3 border-b border-border p-3">
							<div class="flex items-start gap-3">
								<ShadSwitch v-model:model-value="form.allow_all_models" :disabled="!canEdit" class="mt-0.5" />
								<div class="space-y-0.5">
									<ShadLabel class="cursor-pointer">{{ store.getTranslation('settings.teams.allow_all_models') }}</ShadLabel>
									<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.teams.allow_all_models_hint') }}</p>
								</div>
							</div>
						</div>

						<div v-if="form.allow_all_models" class="flex items-center gap-2 p-6 text-sm text-muted-foreground">
							<Bot class="h-4 w-4" />
							{{ store.getTranslation('settings.teams.allow_all_models_hint') }}
						</div>

						<template v-else>
							<div class="border-b border-border p-3">
								<div class="relative">
									<Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
									<ShadInput
										:model-value="modelSearch"
										:placeholder="store.getTranslation('settings.teams.search_models')"
										class="pl-8 h-9"
										@update:model-value="onModelSearch(String($event))"
									/>
								</div>
							</div>

							<div class="grid gap-0 md:grid-cols-[16rem_1fr]">
							<div class="border-b border-border p-3 md:border-b-0 md:border-r">
								<h3 class="mb-2 px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
									{{ store.getTranslation('settings.teams.providers') }}
								</h3>
								<div class="max-h-72 space-y-1 overflow-y-auto">
									<label
										v-for="provider in providers"
										:key="provider.id"
										class="flex cursor-pointer items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-accent/40"
									>
										<ShadCheckbox
											:model-value="form.provider_ids.includes(provider.id)"
											:disabled="!canEdit"
											@update:model-value="checked => toggleId(form.provider_ids, provider.id, Boolean(checked))"
										/>
										<span class="truncate text-sm text-foreground">{{ provider.name }}</span>
									</label>
								</div>
							</div>

							<div ref="modelScroll" class="max-h-72 overflow-y-auto p-3">
								<div v-if="modelsLoading" class="flex items-center justify-center py-10 text-muted-foreground">
									<Loader2 class="h-5 w-5 animate-spin" />
								</div>
								<template v-else>
									<p v-if="models.length === 0" class="py-8 text-center text-sm text-muted-foreground">
										{{ store.getTranslation('settings.teams.no_models') }}
									</p>
									<label
										v-for="model in models"
										:key="model.id"
										class="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5"
										:class="isProviderGranted(model.provider.id) ? 'opacity-50' : 'hover:bg-accent/40'"
									>
										<ShadCheckbox
											:model-value="form.model_ids.includes(model.id) || isProviderGranted(model.provider.id)"
											:disabled="!canEdit || isProviderGranted(model.provider.id)"
											@update:model-value="checked => toggleId(form.model_ids, model.id, Boolean(checked))"
										/>
										<div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-muted overflow-hidden">
											<img v-if="model.icon" :src="model.icon" class="h-4 w-4 rounded object-cover" />
											<div
												v-else-if="iconStore.getProviderIcon(model.provider.name, model.model_id)?.type === 'svg'"
												v-html="sanitizeSvg(iconStore.getProviderIcon(model.provider.name, model.model_id)?.icon ?? '')"
												class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full text-muted-foreground"
											/>
											<Bot v-else class="h-4 w-4 text-muted-foreground" />
										</div>
										<span class="min-w-0 flex-1">
											<span class="block truncate text-sm text-foreground">{{ model.display_name }}</span>
											<span class="block truncate text-xs text-muted-foreground">{{ model.provider.name }} · {{ model.model_id }}</span>
										</span>
									</label>
									<div ref="modelSentinel" class="flex h-6 items-center justify-center">
										<Loader2 v-if="modelsLoadingMore" class="h-4 w-4 animate-spin text-muted-foreground" />
									</div>
								</template>
							</div>
						</div>
						</template>
					</ShadTabsContent>
				</ShadTabs>
			</div>

			<div v-else class="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border bg-card py-20 text-center text-muted-foreground">
				<Network class="h-8 w-8 opacity-50" />
				<p class="font-medium text-foreground">{{ store.getTranslation('settings.teams.no_selection_title') }}</p>
				<p class="max-w-xs text-sm">{{ store.getTranslation('settings.teams.no_selection') }}</p>
			</div>
		</div>

		<ShadDialog v-model:open="createOpen">
			<ShadDialogContent class="sm:max-w-[460px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('settings.teams.create') }}</ShadDialogTitle>
				</ShadDialogHeader>
				<div class="space-y-4 py-2">
					<div class="space-y-1.5">
						<ShadLabel>{{ store.getTranslation('settings.teams.name') }}</ShadLabel>
						<ShadInput v-model="createForm.name" />
					</div>
					<div class="space-y-1.5">
						<ShadLabel>{{ store.getTranslation('settings.teams.description_field') }}</ShadLabel>
						<ShadTextarea v-model="createForm.description" rows="3" />
					</div>
					<div class="flex items-start gap-3 rounded-md border border-border/60 bg-muted/30 p-3">
						<ShadSwitch v-model:model-value="createForm.allow_all_models" class="mt-0.5" />
						<div class="space-y-0.5">
							<ShadLabel class="cursor-pointer">{{ store.getTranslation('settings.teams.allow_all_models') }}</ShadLabel>
							<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.teams.allow_all_models_hint') }}</p>
						</div>
					</div>
				</div>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="createOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton :disabled="saving || !createForm.name.trim()" @click="createTeam">
						<Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
						{{ store.getTranslation('common.create') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>

		<ShadDialog v-model:open="deleteOpen">
			<ShadDialogContent class="sm:max-w-[440px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('common.delete') }}</ShadDialogTitle>
				</ShadDialogHeader>
				<p class="py-2 text-sm text-muted-foreground">{{ store.getTranslation('settings.teams.confirm_delete') }}</p>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="deleteOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton variant="destructive" :disabled="saving" @click="deleteSelected">
						<Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
						{{ store.getTranslation('common.delete') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</SettingsPage>
</template>

<script setup lang="ts">
import {ADMIN_TEAMS_EDIT} from '~/types/generated/permissions';
import {computed, onMounted, onUnmounted, reactive, ref, watch} from 'vue';
import {Bot, Loader2, Network, Plus, Search, Trash2, Users} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {sanitizeSvg} from '@/composables/useMarkdown';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';
import type {ProviderResponse as ProviderListItem, ModelListAdmin, TeamDetailed, TeamList, TeamRole, UserListResponse} from '~/types/chat';

const store = useMainStore();
const iconStore = useIconsStore();

const teams = ref<TeamList[]>([]);
const selectedTeam = ref<TeamDetailed | null>(null);
const users = ref<UserListResponse[]>([]);
const providers = ref<ProviderListItem[]>([]);
const search = ref('');
const memberSearch = ref('');
const activeTab = ref('members');
const loadingTeams = ref(false);
const saving = ref(false);
const createOpen = ref(false);
const deleteOpen = ref(false);

const form = reactive({
	name: '',
	description: '',
	allow_all_models: false,
	default_model_id: null as string | null,
	member_ids: [] as string[],
	member_roles: {} as Record<string, TeamRole>,
	provider_ids: [] as string[],
	model_ids: [] as string[],
});
const createForm = reactive({name: '', description: '', allow_all_models: false});

const canEdit = computed(() => store.hasPermission(ADMIN_TEAMS_EDIT));

const settingsForm = useSettingsForm({
	value: () => form,
	restore: saved => Object.assign(form, saved),
	canEdit: () => canEdit.value && selectedTeam.value !== null,
	normalize: value => ({
		name: value.name,
		description: value.description,
		allow_all_models: value.allow_all_models,
		default_model_id: value.default_model_id,
		members: [...value.member_ids].sort().map(id => `${id}:${value.member_roles[id] ?? 'member'}`),
		provider_ids: [...value.provider_ids].sort(),
		model_ids: [...value.model_ids].sort(),
	}),
	savedKey: 'settings.teams.save_success',
	errorKey: 'settings.teams.save_error',
	save: saveTeam,
});

const filteredUsers = computed(() => {
	const query = memberSearch.value.trim().toLowerCase();
	if (!query) return users.value;
	return users.value.filter(user => user.username.toLowerCase().includes(query) || user.email.toLowerCase().includes(query));
});

function isProviderGranted(providerId: string) {
	return form.provider_ids.includes(providerId);
}

// --- Lazy-loaded models (admin) ---
const {
	models,
	loading: modelsLoading,
	loadingMore: modelsLoadingMore,
	searchQuery: modelSearch,
	canLoadMore: canLoadMoreModels,
	loadInitial: loadModelsInitial,
	loadMore: loadMoreModels,
	onSearchInput: onModelSearch,
} = useModelPicker<ModelListAdmin>({endpoint: adminModelsPath});
const modelScroll = ref<HTMLElement | null>(null);
const modelSentinel = ref<HTMLElement | null>(null);
let modelObserver: IntersectionObserver | null = null;

function setupModelObserver() {
	teardownModelObserver();
	if (!modelSentinel.value || !modelScroll.value) return;
	modelObserver = new IntersectionObserver(
		entries => {
			for (const entry of entries) {
				if (entry.isIntersecting && canLoadMoreModels.value) loadMoreModels();
			}
		},
		{root: modelScroll.value, rootMargin: '80px'}
	);
	modelObserver.observe(modelSentinel.value);
}

function teardownModelObserver() {
	if (modelObserver) {
		modelObserver.disconnect();
		modelObserver = null;
	}
}

// Load models the first time the Models tab is opened for a restricted team.
watch([activeTab, () => form.allow_all_models], ([tab, allowAll]) => {
	if (tab === 'models' && !allowAll && models.value.length === 0 && !modelsLoading.value) {
		loadModelsInitial();
	}
});

// Attach the infinite-scroll observer whenever the sentinel actually mounts.
// The tab content is mounted asynchronously by reka-ui, so a nextTick after the
// tab switch is not reliable — watching the ref is.
watch(modelSentinel, el => {
	if (el) setupModelObserver();
	else teardownModelObserver();
});

// --- Team detail ---
function applySelected(team: TeamDetailed) {
	form.name = team.name;
	form.description = team.description ?? '';
	form.allow_all_models = team.allow_all_models;
	form.default_model_id = team.default_model_id ?? null;
	form.member_ids = team.members.map(member => member.id);
	form.member_roles = Object.fromEntries(team.members.map(member => [member.id, member.role]));
	form.provider_ids = [...team.model_access.provider_ids];
	form.model_ids = [...team.model_access.model_ids];
}

let searchTimer: ReturnType<typeof setTimeout> | null = null;
watch(search, () => {
	if (searchTimer) clearTimeout(searchTimer);
	searchTimer = setTimeout(loadTeams, 300);
});

async function loadTeams() {
	loadingTeams.value = true;
	try {
		const result = await adminTeams({size: 100, search: search.value});
		teams.value = result.items ?? [];
		if (!selectedTeam.value && teams.value.length > 0) {
			await loadTeam(teams.value[0].id);
		}
	} catch (e: any) {
		store.toast(store.getTranslation('settings.teams.load_error'), {type: 'error', description: e?.message});
	} finally {
		loadingTeams.value = false;
	}
}

async function loadUsers() {
	const result = await adminUsers({size: 100});
	users.value = result.items ?? [];
}

async function loadProviders() {
	const result = (await adminProviders()) as ProviderListItem[];
	providers.value = (result ?? []).filter(p => p.is_enabled).sort((a, b) => a.name.localeCompare(b.name));
}

async function loadTeam(id: string) {
	try {
		const team = await adminTeam(id);
		selectedTeam.value = team;
		applySelected(team);
		settingsForm.markSaved();
	} catch (e: any) {
		store.toast(store.getTranslation('settings.teams.load_error'), {type: 'error', description: e?.message});
	}
}

async function selectTeam(id: string) {
	if (id === selectedTeam.value?.id || !(await settingsForm.confirmDiscard())) return;
	await loadTeam(id);
}

function toggleId(list: string[], id: string, checked: boolean) {
	if (checked && !list.includes(id)) list.push(id);
	if (!checked) {
		const index = list.indexOf(id);
		if (index >= 0) list.splice(index, 1);
	}
}

function toggleMember(userId: string, checked: boolean) {
	toggleId(form.member_ids, userId, checked);
}

async function openCreate() {
	if (!(await settingsForm.confirmDiscard())) return;
	createForm.name = '';
	createForm.description = '';
	createForm.allow_all_models = false;
	createOpen.value = true;
}

async function createTeam() {
	saving.value = true;
	try {
		const team = await createAdminTeam({name: createForm.name.trim(), description: createForm.description || null, allow_all_models: createForm.allow_all_models});
		store.toast(store.getTranslation('settings.teams.create_success'), {type: 'success'});
		createOpen.value = false;
		await loadTeams();
		await loadTeam(team.id);
	} catch (e: any) {
		store.toast(store.getTranslation('settings.teams.create_error'), {type: 'error', description: e?.message});
	} finally {
		saving.value = false;
	}
}

async function saveTeam() {
	if (!selectedTeam.value) return;
	const updated = await putAdminTeam(selectedTeam.value.id, '', {
		name: form.name,
		description: form.description || null,
		allow_all_models: form.allow_all_models,
		default_model_id: form.default_model_id,
		members: form.member_ids.map(id => ({user_id: id, role: form.member_roles[id] ?? 'member'})),
		provider_ids: form.provider_ids,
		model_ids: form.model_ids,
	});
	selectedTeam.value = updated;
	applySelected(updated);
	await loadTeams();
}

async function deleteSelected() {
	if (!selectedTeam.value || selectedTeam.value.is_default) return;
	saving.value = true;
	try {
		await deleteAdminTeam(selectedTeam.value.id);
		store.toast(store.getTranslation('settings.teams.delete_success'), {type: 'success'});
		deleteOpen.value = false;
		selectedTeam.value = null;
		settingsForm.markSaved();
		await loadTeams();
	} catch (e: any) {
		store.toast(store.getTranslation('settings.teams.delete_error'), {type: 'error', description: e?.message});
	} finally {
		saving.value = false;
	}
}

onMounted(async () => {
	await Promise.all([loadUsers(), loadProviders(), loadTeams()]);
});

onUnmounted(teardownModelObserver);
</script>
