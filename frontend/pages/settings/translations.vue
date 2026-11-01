<template>
	<SettingsPage :title="store.getTranslation('settings.translations.title')" :description="store.getTranslation('settings.translations.description')">
		<div class="mb-4 flex flex-wrap items-center gap-3">
			<div class="relative min-w-[220px] flex-1">
				<Search class="absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
				<ShadInput v-model="search" class="pl-10" :placeholder="store.getTranslation('settings.translations.search')" data-testid="translations-search" />
			</div>
			<ShadSelect v-model="language">
				<ShadSelectTrigger class="w-[140px]" data-testid="translations-language">
					<ShadSelectValue />
				</ShadSelectTrigger>
				<ShadSelectContent>
					<ShadSelectItem value="en">{{ store.getTranslation('settings.appearance.language_en') }}</ShadSelectItem>
					<ShadSelectItem value="de">{{ store.getTranslation('settings.appearance.language_de') }}</ShadSelectItem>
				</ShadSelectContent>
			</ShadSelect>
			<label class="flex items-center gap-2 text-sm text-muted-foreground">
				<ShadSwitch v-model="editedOnly" data-testid="translations-edited-only" />
				{{ store.getTranslation('settings.translations.overrides_only') }}
			</label>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>
		<div v-else-if="filtered.length === 0" class="py-12 text-center text-sm text-muted-foreground">
			{{ store.getTranslation('settings.translations.empty') }}
		</div>
		<div v-else class="divide-y divide-border rounded-lg border border-border bg-card">
			<button
				v-for="row in visible"
				:key="row.id"
				type="button"
				class="block w-full px-4 py-3 text-left transition-colors hover:bg-accent/50 disabled:cursor-default disabled:hover:bg-transparent"
				:disabled="!canEdit"
				:data-testid="`translation-${row.key_path}`"
				@click="openEditor(row)"
			>
				<div class="flex items-center gap-2">
					<code class="truncate text-xs text-muted-foreground" data-allow-key>{{ row.key_path }}</code>
					<ShadBadge v-if="row.is_override" variant="secondary" class="shrink-0 text-xs">{{ store.getTranslation('settings.translations.edited') }}</ShadBadge>
				</div>
				<p class="mt-1 text-sm text-foreground" data-allow-key>{{ row.value }}</p>
			</button>
		</div>
		<div v-if="filtered.length > visible.length" class="mt-4 flex justify-center">
			<ShadButton variant="outline" size="sm" data-testid="translations-show-more" @click="limit += PAGE_SIZE">{{ store.getTranslation('settings.translations.show_more') }}</ShadButton>
		</div>

		<ShadDialog v-model:open="editorOpen">
			<ShadDialogContent class="sm:max-w-[560px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('settings.translations.edit') }}</ShadDialogTitle>
					<ShadDialogDescription class="break-all" data-allow-key>{{ editing?.key_path }}</ShadDialogDescription>
				</ShadDialogHeader>
				<div class="space-y-2 py-2">
					<ShadTextarea v-model="editorValue" rows="4" data-testid="translation-value" />
					<p class="text-xs text-muted-foreground" data-allow-key>{{ store.getTranslation('settings.translations.placeholders_hint') }}</p>
				</div>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="editorOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton :disabled="saving || editorValue.trim() === ''" data-testid="translation-save" @click="save">{{ store.getTranslation('common.save') }}</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</SettingsPage>
</template>

<script setup lang="ts">
import {computed, onMounted, ref, watch} from 'vue';
import {Loader2, Search} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {ADMIN_I18N_EDIT} from '~/types/generated/permissions';
import type {Translation} from '~/types/generated/Translation';
import type {TranslationsResponse} from '~/types/generated/TranslationsResponse';

const PAGE_SIZE = 100;

const store = useMainStore();
const canEdit = computed(() => store.hasPermission(ADMIN_I18N_EDIT));

const rows = ref<Translation[]>([]);
const loading = ref(true);
const search = ref('');
const language = ref(store.base?.language ?? 'en');
const editedOnly = ref(false);
const limit = ref(PAGE_SIZE);

const editorOpen = ref(false);
const editing = ref<Translation | null>(null);
const editorValue = ref('');
const saving = ref(false);

const filtered = computed(() => {
	const query = search.value.trim().toLowerCase();
	return rows.value.filter(row => {
		if (row.language !== language.value) return false;
		if (editedOnly.value && !row.is_override) return false;
		return !query || row.key_path.toLowerCase().includes(query) || row.value.toLowerCase().includes(query);
	});
});

const visible = computed(() => filtered.value.slice(0, limit.value));

watch([search, language, editedOnly], () => {
	limit.value = PAGE_SIZE;
});

function openEditor(row: Translation) {
	editing.value = row;
	editorValue.value = row.value;
	editorOpen.value = true;
}

function updateLoadedText(row: Translation) {
	const parts = row.key_path.split('.');
	const last = parts.pop();
	let current = store.base?.i18n?.[row.language];
	for (const part of parts) current = current?.[part];
	if (current && last) current[last] = row.value;
}

async function save() {
	const row = editing.value;
	if (!row) return;
	saving.value = true;
	try {
		await saveTranslation({language: row.language, key_path: row.key_path, value: editorValue.value, is_override: true});
		row.value = editorValue.value;
		row.is_override = true;
		updateLoadedText(row);
		editorOpen.value = false;
		store.toast(store.getTranslation('settings.translations.saved'), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('settings.translations.save_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}

onMounted(async () => {
	try {
		rows.value = (await adminTranslations<TranslationsResponse>()).translations;
	} catch (e: any) {
		store.toast(store.getTranslation('settings.translations.load_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		loading.value = false;
	}
});
</script>
