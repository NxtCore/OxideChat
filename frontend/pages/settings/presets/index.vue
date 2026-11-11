<template>
	<SettingsPage :title="t('title')" :description="t('description')">
		<template v-if="canCreate" #actions>
			<ShadButton data-testid="preset-create" @click="router.push(newPresetPath)">
				<Plus class="h-4 w-4" />
				{{ t('create') }}
			</ShadButton>
		</template>

		<div v-if="!presetStore.loaded" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<template v-else>
			<ShadTabs v-if="visibleTabs.length > 1" v-model="currentTab" class="mb-5 w-full">
				<ShadTabsList>
					<ShadTabsTrigger v-for="tab in visibleTabs" :key="tab" :value="tab" :data-testid="`presets-tab-${tab}`">{{ t(`tabs.${tab}`) }}</ShadTabsTrigger>
				</ShadTabsList>
			</ShadTabs>

			<div v-if="currentTab === 'mine' && rows.length === 0" class="flex flex-col items-center rounded-xl border border-dashed border-border px-6 py-12 text-center" data-testid="presets-empty">
				<Wand2 class="mb-4 h-10 w-10 text-primary" />
				<p class="font-medium text-foreground">{{ t('empty_title') }}</p>
				<p class="mt-1 max-w-md text-sm text-muted-foreground">{{ t('empty_description') }}</p>
				<div class="mt-5 flex flex-wrap justify-center gap-2">
					<ShadButton v-if="canCreate" @click="router.push(newPresetPath)">
						<Plus class="h-4 w-4" />
						{{ t('create') }}
					</ShadButton>
					<ShadButton variant="outline" @click="router.push('/')">
						<MessageSquare class="h-4 w-4" />
						{{ t('start_from_chat') }}
					</ShadButton>
				</div>
				<p class="mt-3 text-xs text-muted-foreground">{{ t('start_from_chat_hint') }}</p>
			</div>

			<div v-else-if="rows.length === 0" class="rounded-xl border border-dashed border-border px-6 py-12 text-center text-sm text-muted-foreground" data-testid="presets-empty-shared">
				{{ t(canCreate ? 'empty_shared_editable' : 'empty_shared') }}
			</div>

			<div v-else class="space-y-2">
				<p v-if="currentTab !== 'mine' && !canCreate" class="text-xs text-muted-foreground">{{ t('read_only') }}</p>
				<div
					v-for="preset in rows"
					:key="preset.id"
					data-testid="preset-row"
					:data-preset-key="preset.key"
					class="flex items-center gap-4 rounded-lg border border-border bg-card px-4 py-3"
				>
					<div class="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-md bg-primary/10">
						<img v-if="preset.icon" :src="preset.icon" alt="" class="h-full w-full object-cover" />
						<Wand2 v-else class="h-5 w-5 text-primary" />
					</div>
					<NuxtLink :to="`/settings/presets/${preset.id}`" class="min-w-0 flex-1">
						<div class="flex flex-wrap items-center gap-2">
							<span class="font-medium text-foreground">{{ preset.name }}</span>
							<span v-if="preset.scope === 'team'" class="rounded bg-primary/10 px-1.5 py-0.5 text-xs text-primary" data-testid="preset-row-scope">
								{{ presetScopeLabel(preset, store.getTranslation) }}
							</span>
							<span v-if="preset.unavailable" class="rounded-full bg-destructive/10 px-2 py-0.5 text-xs text-destructive">
								{{ store.getTranslation(`presets.unavailable.${preset.unavailable}`) }}
							</span>
						</div>
						<p class="truncate text-sm text-muted-foreground">
							{{ preset.base_model ? store.getTranslation('presets.picker.on_model', {model: preset.base_model.display_name}) : store.getTranslation('presets.unavailable.base_model_unavailable') }}
						</p>
					</NuxtLink>
					<button
						class="hidden items-center gap-1 rounded-md px-2 py-1 font-mono text-xs text-muted-foreground hover:bg-accent sm:inline-flex"
						:title="t('copy_id')"
						@click="store.copyToClipboard(preset.key)"
					>
						{{ preset.key }}
						<Copy class="h-3 w-3" />
					</button>
					<ShadSwitch
						v-if="preset.can_edit"
						data-testid="preset-row-enabled"
						:model-value="preset.is_enabled"
						:disabled="preset.disabled_by_admin && !scopes?.installation"
						:aria-label="t('enabled')"
						@update:model-value="value => setEnabled(preset, Boolean(value))"
					/>
					<ShadDropdownMenu>
						<ShadDropdownMenuTrigger as-child>
							<ShadButton variant="ghost" size="icon" class="h-8 w-8" data-testid="preset-row-menu" :aria-label="t('actions')">
								<MoreHorizontal class="h-4 w-4" />
							</ShadButton>
						</ShadDropdownMenuTrigger>
						<ShadDropdownMenuContent align="end" class="border-border bg-popover">
							<ShadDropdownMenuItem class="text-popover-foreground focus:bg-accent" @click="router.push(`/settings/presets/${preset.id}`)">
								<Pencil v-if="preset.can_edit" class="mr-2 h-4 w-4" />
								<Eye v-else class="mr-2 h-4 w-4" />
								{{ t(preset.can_edit ? 'edit' : 'view') }}
							</ShadDropdownMenuItem>
							<ShadDropdownMenuItem v-if="hasAnyScope" data-testid="preset-row-duplicate" class="text-popover-foreground focus:bg-accent" @click="duplicate(preset)">
								<CopyPlus class="mr-2 h-4 w-4" />
								{{ t('duplicate') }}
							</ShadDropdownMenuItem>
							<template v-if="preset.can_edit">
								<ShadDropdownMenuSeparator class="bg-border" />
								<ShadDropdownMenuItem data-testid="preset-row-delete" class="text-destructive focus:bg-destructive/10" @click="pendingDelete = preset">
									<Trash2 class="mr-2 h-4 w-4" />
									{{ t('delete') }}
								</ShadDropdownMenuItem>
							</template>
						</ShadDropdownMenuContent>
					</ShadDropdownMenu>
				</div>
			</div>
		</template>

		<SettingsPresetsDeleteDialog :preset="pendingDelete" @close="pendingDelete = null" />
	</SettingsPage>
</template>

<script setup lang="ts">
import {Copy, CopyPlus, Eye, Loader2, MessageSquare, MoreHorizontal, Pencil, Plus, Trash2, Wand2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {usePresetStore} from '~/stores/presetStore';
import type {PresetResponse, PresetScope} from '~/types/presets';

type PresetTab = 'mine' | 'team' | 'organization';

const TAB_SCOPES: Record<PresetTab, PresetScope> = {mine: 'personal', team: 'team', organization: 'installation'};

const store = useMainStore();
const presetStore = usePresetStore();
const router = useRouter();
const route = useRoute();

const t = (key: string) => store.getTranslation(`presets.settings.${key}`);

const pendingDelete = ref<PresetResponse | null>(null);

const scopes = computed(() => presetStore.scopes);
const hasAnyScope = computed(() => !!scopes.value && (scopes.value.personal || scopes.value.teams.length > 0 || scopes.value.installation));

const visibleTabs = computed<PresetTab[]>(() => {
	const has = (scope: PresetScope) => presetStore.presets.some(preset => preset.scope === scope);
	const tabs: PresetTab[] = ['mine'];
	if ((scopes.value?.teams.length ?? 0) > 0 || has('team')) tabs.push('team');
	if (scopes.value?.installation || has('installation')) tabs.push('organization');
	return tabs;
});

const queryTab = route.query.tab as PresetTab | undefined;
const currentTab = ref<PresetTab>(queryTab && queryTab in TAB_SCOPES ? queryTab : 'mine');

watch(currentTab, tab => router.replace({query: tab === 'mine' ? {} : {tab}}));

const rows = computed(() => presetStore.presets.filter(preset => preset.scope === TAB_SCOPES[currentTab.value]));

const canCreate = computed(() => {
	if (!scopes.value) return false;
	if (currentTab.value === 'team') return scopes.value.teams.length > 0;
	if (currentTab.value === 'organization') return scopes.value.installation;
	return scopes.value.personal;
});

const newPresetPath = computed(() => `/settings/presets/new?scope=${TAB_SCOPES[currentTab.value]}`);

onMounted(() => Promise.all([presetStore.ensurePresets(), presetStore.ensureScopes()]));

async function setEnabled(preset: PresetResponse, enabled: boolean) {
	try {
		await presetStore.updatePreset(preset.id, {...presetToRequest(preset), is_enabled: enabled});
		store.toast(t(enabled ? 'enabled_toast' : 'disabled_toast'), {type: 'success'});
	} catch (e: any) {
		store.toast(t('save_error'), {type: 'error', description: errorMessage(e, '')});
	}
}

function duplicate(preset: PresetResponse) {
	presetStore.draft = {...presetToRequest(preset), slug: '', name: store.getTranslation('presets.copy_name', {name: preset.name}), base_model: preset.base_model};
	router.push('/settings/presets/new');
}
</script>
