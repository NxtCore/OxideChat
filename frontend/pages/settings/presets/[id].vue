<template>
	<SettingsPage :title="pageTitle" :description="pageDescription" :form="settingsForm">
		<template #leading>
			<ShadButton variant="ghost" size="icon" class="shrink-0" data-testid="preset-editor-back" @click="router.push('/settings/presets')">
				<ArrowLeft class="h-4 w-4" />
			</ShadButton>
			<div class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-muted">
				<img v-if="form.icon" :src="form.icon" class="h-full w-full object-cover" alt="" />
				<Wand2 v-else class="h-5 w-5 text-primary" />
			</div>
		</template>
		<template v-if="!isNew && canEdit" #actions>
			<ShadButton variant="outline" data-testid="preset-editor-delete" @click="deleteOpen = true">
				<Trash2 class="h-4 w-4" />
				{{ store.getTranslation('presets.settings.delete') }}
			</ShadButton>
		</template>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="notFound" class="flex items-center justify-center py-12 text-muted-foreground">
			<p>{{ t('not_found') }}</p>
		</div>

		<template v-else>
			<div v-if="unavailable" class="mb-5 flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive" data-testid="preset-broken-banner">
				<AlertTriangle class="mt-0.5 h-4 w-4 shrink-0" />
				<span>{{ store.getTranslation(`presets.editor.broken.${unavailable}`) }}</span>
			</div>

			<ShadTabs v-model="currentTab" class="mb-5 w-full">
				<ShadTabsList>
					<ShadTabsTrigger v-for="tab in tabs" :key="tab" :value="tab" :data-testid="`preset-tab-${tab}`">{{ t(`tabs.${tab}`) }}</ShadTabsTrigger>
				</ShadTabsList>
			</ShadTabs>

			<fieldset :disabled="!canEdit" class="contents">
				<SettingsPresetsGeneralTab
					v-show="currentTab === 'general'"
					:form="form"
					:slug-status="slugStatus"
					:base-model-key="baseModelKey"
					:scope-label="scopeLabel"
					:assignment="canEdit ? assignment : null"
					:assignment-options="assignmentOptions"
					:slug-renamed="slugRenamed"
					:enabled-locked="enabledLocked"
					:disabled="!canEdit"
					@name="onNameInput"
					@slug="onSlugInput"
					@base-key="value => (baseModelKey = value)"
					@base="onBaseSelected"
					@assignment="onAssignment"
				/>
				<div v-show="currentTab === 'instructions'" class="space-y-4">
					<SettingsModelsPromptTab :form="form" title-key="presets.editor.instructions_title" description-key="presets.editor.instructions_description" />
					<div class="flex items-center justify-between rounded-lg border border-border p-3">
						<div class="space-y-0.5">
							<ShadLabel>{{ t('prompt_visible') }}</ShadLabel>
							<p class="text-xs text-muted-foreground">{{ t('prompt_visible_description') }}</p>
						</div>
						<ShadSwitch v-model:model-value="form.prompt_visible" />
					</div>
				</div>
				<SettingsModelsParametersTab v-show="currentTab === 'parameters'" :form="form" :capabilities="baseModel?.capabilities ?? []" />
				<SettingsPresetsToolsTab v-show="currentTab === 'tools'" :form="form" :capabilities="baseModel?.capabilities ?? null" />
				<SettingsPresetsStartersTab v-show="currentTab === 'starters'" :form="form" />
			</fieldset>
		</template>

		<SettingsPresetsDeleteDialog :preset="deleteOpen ? preset : null" @close="deleteOpen = false" @deleted="onDeleted" />
	</SettingsPage>
</template>

<script setup lang="ts">
import {AlertTriangle, ArrowLeft, Loader2, Trash2, Wand2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {usePresetStore} from '~/stores/presetStore';
import type {AssignmentOption, AssignmentTarget} from '~/types/assignments';
import type {ModelList} from '~/types/chat';
import type {PresetResponse, PresetUnavailable, SavePresetRequest} from '~/types/presets';
import type {SlugStatus} from '~/components/settings/presets/GeneralTab.vue';

definePageMeta({key: route => route.fullPath});

const store = useMainStore();
const presetStore = usePresetStore();
const route = useRoute();
const router = useRouter();

const t = (key: string) => store.getTranslation(`presets.editor.${key}`);

const tabs = ['general', 'instructions', 'parameters', 'tools', 'starters'] as const;
const samplingKeys = ['temperature', 'max_tokens', 'top_p', 'top_k', 'frequency_penalty', 'presence_penalty', 'seed', 'stop', 'logprobs', 'top_logprobs', 'parallel_tool_calls'] as const;

const presetId = route.params.id as string;
const isNew = presetId === 'new';
const preset = ref<PresetResponse | null>(null);
const loading = ref(!isNew);
const notFound = ref(false);
const currentTab = ref<(typeof tabs)[number]>('general');
const deleteOpen = ref(false);
const baseModel = ref<ModelList | null>(null);
const baseModelKey = ref<string | null>(null);
const slugTouched = ref(!isNew);
const slugStatus = ref<SlugStatus>('idle');

const form = reactive({
	name: '',
	slug: '',
	description: '',
	icon: '' as string | null,
	is_enabled: true,
	base_model_id: '',
	system_prompt: '',
	prompt_visible: true,
	sampling: Object.fromEntries(samplingKeys.map(key => [key, null])) as Record<(typeof samplingKeys)[number], any>,
	reasoning_effort: null as string | null,
	reasoning_budget_tokens: null as number | null,
	tool_ids: [] as string[],
	tools_locked: false,
	starters: ['', '', '', ''] as string[],
});

const {data: storedAssignment, load: loadAssignment, replace: replaceAssignment} = useAssignments();
const newAssignment = ref<AssignmentTarget[]>([]);
const assignment = computed(() => (isNew ? newAssignment.value : (storedAssignment.value?.targets ?? null)));
const assignmentPath = `/api/v1/presets/${presetId}/assignments`;
const assignmentOptions = computed<AssignmentOption[]>(() => {
	const scopes = presetStore.scopes;
	if (!scopes) return [];
	return [
		...(scopes.personal ? [{kind: 'user' as const, id: store.user?.id ?? null, label: store.getTranslation('presets.scope.personal')}] : []),
		...scopes.teams.map(team => ({kind: 'team' as const, id: team.id, label: team.name})),
		...(scopes.installation ? [{kind: 'everyone' as const, id: null, label: ''}] : []),
	];
});
const canEdit = computed(() => (isNew ? assignmentOptions.value.length > 0 : !!preset.value?.can_edit));
const scopeLabel = computed(() => (preset.value ? presetScopeLabel(preset.value, store.getTranslation) : ''));
const slugRenamed = computed(() => !!preset.value?.has_gateway_usage && preset.value.slug !== form.slug);
const enabledLocked = computed(() => !!preset.value?.disabled_by_admin && !presetStore.scopes?.installation);
const unavailable = computed<PresetUnavailable | null>(() => (preset.value?.unavailable === 'disabled' ? null : (preset.value?.unavailable ?? null)));
const pageTitle = computed(() => (isNew ? t('new_title') : form.name || preset.value?.name || t('loading')));
const pageDescription = computed(() => {
	if (isNew && !form.slug) return t('new_description');
	const key = presetKey(form.slug || '…');
	return baseModel.value ? `${baseModel.value.display_name} • ${key}` : key;
});

const settingsForm = useSettingsForm({
	value: () => form,
	restore: saved => Object.assign(form, saved),
	savedKey: 'presets.editor.saved',
	errorKey: 'presets.editor.save_error',
	canEdit: () => canEdit.value,
	validate: () => {
		if (!form.name.trim()) return 'presets.editor.name_required';
		if (!isValidPresetSlug(form.slug)) return 'presets.editor.slug_invalid';
		if (slugStatus.value === 'taken') return 'presets.editor.slug_taken';
		if (!form.base_model_id) return 'presets.editor.model_required';
		return null;
	},
	save: savePreset,
});

function populate(source: SavePresetRequest) {
	form.name = source.name;
	form.slug = source.slug;
	form.description = source.description ?? '';
	form.icon = source.icon ?? '';
	form.is_enabled = source.is_enabled;
	form.base_model_id = source.base_model_id;
	form.system_prompt = source.system_prompt ?? '';
	form.prompt_visible = source.prompt_visible;
	for (const key of samplingKeys) form.sampling[key] = source.sampling[key] ?? null;
	form.sampling.stop = Array.isArray(source.sampling.stop) && source.sampling.stop.length > 0 ? [...source.sampling.stop] : null;
	form.reasoning_effort = source.reasoning.effort ?? null;
	form.reasoning_budget_tokens = source.reasoning.budget_tokens ?? null;
	form.tool_ids = [...source.tool_ids];
	form.tools_locked = source.tools_locked;
	form.starters = [...source.starters, '', '', '', ''].slice(0, MAX_PRESET_STARTERS);
}

function buildRequest(): SavePresetRequest {
	const sampling = Object.fromEntries(samplingKeys.map(key => [key, form.sampling[key]]).filter(([, value]) => value !== null && !(Array.isArray(value) && value.length === 0)));
	const optional = (value: string) => (value.trim() ? value.trim() : null);
	return {
		slug: form.slug.trim(),
		name: form.name.trim(),
		description: optional(form.description),
		icon: optional(form.icon ?? ''),
		base_model_id: form.base_model_id,
		system_prompt: optional(form.system_prompt),
		sampling,
		reasoning: {effort: form.reasoning_effort, budget_tokens: form.reasoning_budget_tokens},
		tool_ids: [...form.tool_ids],
		tools_locked: form.tools_locked,
		starters: form.starters.map(starter => starter.trim()).filter(Boolean),
		prompt_visible: form.prompt_visible,
		is_enabled: form.is_enabled,
		...(isNew ? scopeRequest() : {}),
	};
}

function scopeRequest(): Pick<SavePresetRequest, 'scope' | 'team_id'> {
	const target = newAssignment.value[0];
	if (target?.kind === 'team' && target.id) return {scope: 'team', team_id: target.id};
	return {scope: target?.kind === 'everyone' ? 'installation' : 'personal'};
}

function initialAssignment() {
	const kind = route.query.scope === 'team' ? 'team' : route.query.scope === 'installation' ? 'everyone' : 'user';
	const option = assignmentOptions.value.find(candidate => candidate.kind === kind) ?? assignmentOptions.value[0];
	newAssignment.value = option ? [{...option, inherited: false}] : [];
}

async function onAssignment(targets: AssignmentTarget[]) {
	if (isNew) {
		newAssignment.value = targets;
		return;
	}
	if (await replaceAssignment(assignmentPath, targets)) preset.value = await presetStore.fetchPreset(presetId);
}

function applyPreset(loaded: PresetResponse) {
	preset.value = loaded;
	baseModel.value = loaded.base_model;
	baseModelKey.value = loaded.base_model?.model_id ?? null;
	populate(presetToRequest(loaded));
}

async function savePreset() {
	const body = buildRequest();
	if (isNew) {
		const created = await presetStore.createPreset(body);
		setTimeout(() => router.replace(`/settings/presets/${created.id}`), 0);
		return;
	}
	applyPreset(await presetStore.updatePreset(presetId, body));
}

let slugTimer: ReturnType<typeof setTimeout> | null = null;

function checkSlug() {
	if (slugTimer) clearTimeout(slugTimer);
	const slug = form.slug;
	if (!slug) {
		slugStatus.value = 'idle';
		return;
	}
	if (!isValidPresetSlug(slug)) {
		slugStatus.value = 'invalid';
		return;
	}
	if (!isNew && slug === preset.value?.slug) {
		slugStatus.value = 'available';
		return;
	}
	slugStatus.value = 'checking';
	slugTimer = setTimeout(async () => {
		try {
			const result = await presetStore.checkSlug(slug, isNew ? undefined : presetId);
			if (form.slug === slug) slugStatus.value = !result.valid ? 'invalid' : result.available ? 'available' : 'taken';
		} catch {
			if (form.slug === slug) slugStatus.value = 'idle';
		}
	}, 300);
}

function onNameInput(value: string) {
	form.name = value;
	if (!slugTouched.value) {
		form.slug = slugify(value);
		checkSlug();
	}
}

function onSlugInput(value: string) {
	form.slug = value;
	slugTouched.value = true;
	checkSlug();
}

function onBaseSelected(model: ModelList | null) {
	baseModel.value = model;
	form.base_model_id = model?.id ?? '';
}

function onDeleted() {
	settingsForm.markSaved();
	router.push('/settings/presets');
}

onMounted(async () => {
	if (isNew) {
		await presetStore.ensureScopes();
		initialAssignment();
		settingsForm.markSaved();
		const draft = presetStore.draft;
		presetStore.draft = null;
		if (draft) {
			populate(draft);
			slugTouched.value = false;
			form.slug = slugify(form.name);
			checkSlug();
			baseModel.value = draft.base_model ?? null;
			baseModelKey.value = draft.base_model?.model_id ?? null;
		}
		return;
	}
	try {
		applyPreset(await presetStore.fetchPreset(presetId));
		if (preset.value?.can_edit) await Promise.all([presetStore.ensureScopes(), loadAssignment(assignmentPath)]);
		else if (preset.value?.disabled_by_admin) await presetStore.ensureScopes();
		slugStatus.value = 'available';
		settingsForm.markSaved();
	} catch {
		notFound.value = true;
	} finally {
		loading.value = false;
	}
});
</script>
