<template>
	<div class="w-full lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between mb-6">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tabs.teams') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.teams.description') }}</p>
			</div>
			<div class="flex items-center gap-2">
				<template v-if="canEdit && selectedTeam">
					<ShadButton
						v-if="!selectedTeam.is_default"
						variant="outline"
						size="sm"
						class="gap-2 text-destructive hover:text-destructive"
						:disabled="saving"
						@click="deleteOpen = true"
					>
						<Trash2 class="h-4 w-4" />
					</ShadButton>
					<ShadButton variant="default" size="sm" :disabled="saving" @click="saveAll">
						<Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
						{{ store.getTranslation('common.save') }}
					</ShadButton>
				</template>
				<ShadButton v-if="canEdit" variant="default" size="sm" class="gap-2" @click="openCreate">
					<Plus class="h-4 w-4" />
					<span>{{ store.getTranslation('settings.teams.create') }}</span>
				</ShadButton>
			</div>
		</div>

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
							<span class="shrink-0 text-xs text-muted-foreground">{{ store.getTranslation('settings.teams.selected_count', {count: selectedMemberIds.length}) }}</span>
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
									:model-value="selectedMemberIds.includes(user.id)"
									:disabled="!canEdit || selectedTeam.is_default"
									@update:model-value="checked => toggleId(selectedMemberIds, user.id, Boolean(checked))"
								/>
								<span class="min-w-0 flex-1">
									<span class="block truncate text-sm text-foreground">{{ user.username }}</span>
									<span class="block truncate text-xs text-muted-foreground">{{ user.email }}</span>
								</span>
							</label>
						</div>
					</ShadTabsContent>

					<!-- Models -->
					<ShadTabsContent value="models" class="mt-4 rounded-lg border border-border bg-card">
						<div class="border-b border-border p-3 space-y-1.5">
							<ShadLabel>{{ store.getTranslation('settings.teams.default_model') }}</ShadLabel>
							<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.teams.default_model_hint') }}</p>
							<DefaultModelPicker
								v-model="teamDefaultModelId"
								:disabled="!canEdit"
								endpoint="/api/v1/admin/models"
								selected-model-endpoint="/api/v1/admin/models"
								value-mode="uuid"
								:placeholder="store.getTranslation('settings.teams.use_global_default')"
								@update:model-value="v => (teamDefaultModelId = v)"
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
											:model-value="selectedProviderIds.includes(provider.id)"
											:disabled="!canEdit"
											@update:model-value="checked => toggleId(selectedProviderIds, provider.id, Boolean(checked))"
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
										:class="isProviderGranted(model.provider_id) ? 'opacity-50' : 'hover:bg-accent/40'"
									>
										<ShadCheckbox
											:model-value="selectedModelIds.includes(model.id) || isProviderGranted(model.provider_id)"
											:disabled="!canEdit || isProviderGranted(model.provider_id)"
											@update:model-value="checked => toggleId(selectedModelIds, model.id, Boolean(checked))"
										/>
										<div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-muted overflow-hidden">
											<img v-if="model.icon" :src="model.icon" class="h-4 w-4 rounded object-cover" />
											<div
												v-else-if="iconStore.getProviderIcon(model.provider.name, model.model_id)?.type === 'svg'"
												v-html="iconStore.getProviderIcon(model.provider.name, model.model_id)!.icon"
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
	</div>
</template>

<script setup lang="ts">
import {computed, onMounted, onUnmounted, reactive, ref, watch} from 'vue';
import {Bot, Loader2, Network, Plus, Search, Trash2, Users} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import DefaultModelPicker from '~/components/settings/DefaultModelPicker.vue';
import type {ModelListAdmin, PaginatedResponse, TeamDetailed, TeamList} from '~/types/chat';

interface UserListItem {
	id: string;
	email: string;
	username: string;
}

interface PaginatedUsersResponse {
	users: UserListItem[];
}

interface ProviderListItem {
	id: string;
	name: string;
	is_enabled: boolean;
}

const {$customFetch} = useNuxtApp();
const store = useMainStore();
const iconStore = useIconsStore();

const teams = ref<TeamList[]>([]);
const selectedTeam = ref<TeamDetailed | null>(null);
const users = ref<UserListItem[]>([]);
const providers = ref<ProviderListItem[]>([]);
const search = ref('');
const memberSearch = ref('');
const activeTab = ref('members');
const loadingTeams = ref(false);
const saving = ref(false);
const createOpen = ref(false);
const deleteOpen = ref(false);

const form = reactive({name: '', description: '', allow_all_models: false});
const teamDefaultModelId = ref<string | null>(null);
const createForm = reactive({name: '', description: '', allow_all_models: false});
const selectedMemberIds = ref<string[]>([]);
const selectedProviderIds = ref<string[]>([]);
const selectedModelIds = ref<string[]>([]);

const canEdit = computed(() => store.hasPermission('admin.teams.edit'));
const filteredUsers = computed(() => {
	const query = memberSearch.value.trim().toLowerCase();
	if (!query) return users.value;
	return users.value.filter(user => user.username.toLowerCase().includes(query) || user.email.toLowerCase().includes(query));
});

function isProviderGranted(providerId: string) {
	return selectedProviderIds.value.includes(providerId);
}

// --- Lazy-loaded models (admin) ---
const models = ref<ModelListAdmin[]>([]);
const modelSearch = ref('');
const modelPage = ref(1);
const modelPageSize = 30;
const modelsHasMore = ref(false);
const modelsLoading = ref(false);
const modelsLoadingMore = ref(false);
const modelScroll = ref<HTMLElement | null>(null);
const modelSentinel = ref<HTMLElement | null>(null);
let modelObserver: IntersectionObserver | null = null;
let modelSearchTimer: ReturnType<typeof setTimeout> | null = null;

const canLoadMoreModels = computed(() => modelsHasMore.value && !modelsLoading.value && !modelsLoadingMore.value);

async function fetchModels(page: number): Promise<ModelListAdmin[]> {
	const params: Record<string, string> = {page: page.toString(), size: modelPageSize.toString()};
	if (modelSearch.value.trim()) params.query = modelSearch.value.trim();
	const res = await $customFetch<PaginatedResponse<ModelListAdmin>>('/api/v1/admin/models', {params});
	modelsHasMore.value = res?.has_more ?? false;
	return res?.items ?? [];
}

async function loadModelsInitial() {
	modelsLoading.value = true;
	modelPage.value = 1;
	try {
		models.value = await fetchModels(1);
	} catch (e) {
		console.error('Failed to load models:', e);
		models.value = [];
		modelsHasMore.value = false;
	} finally {
		modelsLoading.value = false;
	}
}

async function loadMoreModels() {
	if (!canLoadMoreModels.value) return;
	modelsLoadingMore.value = true;
	const next = modelPage.value + 1;
	try {
		const items = await fetchModels(next);
		models.value.push(...items);
		modelPage.value = next;
	} catch (e) {
		console.error('Failed to load more models:', e);
	} finally {
		modelsLoadingMore.value = false;
	}
}

function onModelSearch(value: string) {
	modelSearch.value = value;
	if (modelSearchTimer) clearTimeout(modelSearchTimer);
	modelSearchTimer = setTimeout(loadModelsInitial, 300);
}

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
	teamDefaultModelId.value = team.default_model_id ?? null;
	selectedMemberIds.value = team.members.map(member => member.id);
	selectedProviderIds.value = [...team.model_access.provider_ids];
	selectedModelIds.value = [...team.model_access.model_ids];
}

let searchTimer: ReturnType<typeof setTimeout> | null = null;
watch(search, () => {
	if (searchTimer) clearTimeout(searchTimer);
	searchTimer = setTimeout(loadTeams, 300);
});

async function loadTeams() {
	loadingTeams.value = true;
	try {
		const result = await $customFetch<PaginatedResponse<TeamList>>('/api/v1/admin/teams', {params: {size: 100, search: search.value}});
		teams.value = result.items ?? [];
		if (!selectedTeam.value && teams.value.length > 0) {
			await selectTeam(teams.value[0].id);
		}
	} catch (e: any) {
		store.toast(store.getTranslation('settings.teams.load_error'), {type: 'error', description: e?.message});
	} finally {
		loadingTeams.value = false;
	}
}

async function loadUsers() {
	const result = await $customFetch<PaginatedUsersResponse>('/api/v1/admin/users', {params: {per_page: 100}});
	users.value = result.users ?? [];
}

async function loadProviders() {
	const result = await $customFetch<ProviderListItem[]>('/api/v1/admin/providers');
	providers.value = (result ?? []).filter(p => p.is_enabled).sort((a, b) => a.name.localeCompare(b.name));
}

async function selectTeam(id: string) {
	try {
		const team = await $customFetch<TeamDetailed>(`/api/v1/admin/teams/${id}`);
		selectedTeam.value = team;
		applySelected(team);
	} catch (e: any) {
		store.toast(store.getTranslation('settings.teams.load_error'), {type: 'error', description: e?.message});
	}
}

function toggleId(list: string[], id: string, checked: boolean) {
	if (checked && !list.includes(id)) list.push(id);
	if (!checked) {
		const index = list.indexOf(id);
		if (index >= 0) list.splice(index, 1);
	}
}

function openCreate() {
	createForm.name = '';
	createForm.description = '';
	createForm.allow_all_models = false;
	createOpen.value = true;
}

async function createTeam() {
	saving.value = true;
	try {
		const team = await $customFetch<TeamDetailed>('/api/v1/admin/teams', {
			method: 'POST',
			body: {name: createForm.name.trim(), description: createForm.description || null, allow_all_models: createForm.allow_all_models},
		});
		store.toast(store.getTranslation('settings.teams.create_success'), {type: 'success'});
		createOpen.value = false;
		await loadTeams();
		await selectTeam(team.id);
	} catch (e: any) {
		store.toast(store.getTranslation('settings.teams.create_error'), {type: 'error', description: e?.message});
	} finally {
		saving.value = false;
	}
}

async function saveAll() {
	if (!selectedTeam.value || !canEdit.value) return;
	saving.value = true;
	try {
		const id = selectedTeam.value.id;

		await $customFetch<TeamDetailed>(`/api/v1/admin/teams/${id}`, {
			method: 'PATCH',
			body: {name: form.name, description: form.description ? form.description : null, allow_all_models: form.allow_all_models, default_model_id: teamDefaultModelId.value},
		});

		await $customFetch<TeamDetailed>(`/api/v1/admin/teams/${id}/members`, {
			method: 'PUT',
			body: {user_ids: selectedMemberIds.value},
		});

		const providerIds = [...selectedProviderIds.value];
		const modelIds = [...selectedModelIds.value];

		const withModels = await $customFetch<TeamDetailed>(`/api/v1/admin/teams/${id}/models`, {
			method: 'PUT',
			body: {provider_ids: providerIds, model_ids: modelIds},
		});

		selectedTeam.value = withModels;
		applySelected(withModels);

		await loadTeams();
		store.toast(store.getTranslation('settings.teams.save_success'), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('settings.teams.save_error'), {type: 'error', description: e?.message});
	} finally {
		saving.value = false;
	}
}

async function deleteSelected() {
	if (!selectedTeam.value || selectedTeam.value.is_default) return;
	saving.value = true;
	try {
		await $customFetch(`/api/v1/admin/teams/${selectedTeam.value.id}`, {method: 'DELETE'});
		store.toast(store.getTranslation('settings.teams.delete_success'), {type: 'success'});
		deleteOpen.value = false;
		selectedTeam.value = null;
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
