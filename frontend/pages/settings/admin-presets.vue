<template>
	<SettingsPage :title="t('title')" :description="t('description')">
		<div class="mb-4 flex flex-wrap items-center gap-3">
			<div class="relative min-w-[220px] flex-1">
				<Search class="absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
				<ShadInput v-model="search" class="pl-10" :placeholder="t('search')" data-testid="admin-presets-search" />
			</div>
			<ShadSelect v-model="scope">
				<ShadSelectTrigger class="w-[170px]" data-testid="admin-presets-scope">
					<ShadSelectValue />
				</ShadSelectTrigger>
				<ShadSelectContent>
					<ShadSelectItem value="all">{{ t('all_scopes') }}</ShadSelectItem>
					<ShadSelectItem v-for="option in SCOPES" :key="option" :value="option">{{ store.getTranslation(`presets.scope.${option}`) }}</ShadSelectItem>
				</ShadSelectContent>
			</ShadSelect>
			<ShadSelect v-if="teams.length > 0" v-model="teamId">
				<ShadSelectTrigger class="w-[200px]" data-testid="admin-presets-team">
					<ShadSelectValue />
				</ShadSelectTrigger>
				<ShadSelectContent>
					<ShadSelectItem value="all">{{ t('all_teams') }}</ShadSelectItem>
					<ShadSelectItem v-for="team in teams" :key="team.id" :value="team.id">{{ team.name }}</ShadSelectItem>
				</ShadSelectContent>
			</ShadSelect>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>
		<div v-else-if="filtered.length === 0" class="py-12 text-center text-sm text-muted-foreground">{{ t('empty') }}</div>
		<div v-else class="divide-y divide-border rounded-lg border border-border bg-card">
			<div v-for="preset in filtered" :key="preset.id" class="flex items-center gap-4 px-4 py-3" data-testid="admin-preset-row" :data-preset-key="preset.key">
				<div class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-md bg-primary/10">
					<img v-if="preset.icon" :src="preset.icon" alt="" class="h-full w-full object-cover" />
					<Wand2 v-else class="h-4 w-4 text-primary" />
				</div>
				<div class="min-w-0 flex-1">
					<div class="flex flex-wrap items-center gap-2">
						<span class="font-medium text-foreground">{{ preset.name }}</span>
						<span class="rounded bg-primary/10 px-1.5 py-0.5 text-xs text-primary">{{ ownerLabel(preset) }}</span>
					</div>
					<p class="truncate font-mono text-xs text-muted-foreground">
						{{ preset.key }}<template v-if="preset.base_model_name"> · {{ preset.base_model_name }}</template>
					</p>
				</div>
				<ShadSwitch
					v-if="canEdit"
					data-testid="admin-preset-enabled"
					:model-value="preset.is_enabled"
					:aria-label="store.getTranslation('presets.settings.enabled')"
					@update:model-value="value => save(preset, {is_enabled: Boolean(value)})"
				/>
				<ShadDropdownMenu v-if="canEdit">
					<ShadDropdownMenuTrigger as-child>
						<ShadButton variant="ghost" size="icon" class="h-8 w-8" data-testid="admin-preset-menu" :aria-label="store.getTranslation('presets.settings.actions')">
							<MoreHorizontal class="h-4 w-4" />
						</ShadButton>
					</ShadDropdownMenuTrigger>
					<ShadDropdownMenuContent align="end" class="border-border bg-popover">
						<ShadDropdownMenuItem v-if="preset.scope !== 'personal'" class="text-popover-foreground focus:bg-accent" @click="router.push(`/settings/presets/${preset.id}`)">
							<Pencil class="mr-2 h-4 w-4" />
							{{ store.getTranslation('presets.settings.edit') }}
						</ShadDropdownMenuItem>
						<ShadDropdownMenuItem data-testid="admin-preset-rename" class="text-popover-foreground focus:bg-accent" @click="openRename(preset)">
							<TextCursorInput class="mr-2 h-4 w-4" />
							{{ t('rename') }}
						</ShadDropdownMenuItem>
					</ShadDropdownMenuContent>
				</ShadDropdownMenu>
			</div>
		</div>

		<ShadDialog :open="!!renaming" @update:open="open => !open && (renaming = null)">
			<ShadDialogContent class="sm:max-w-[480px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ t('rename') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ t('rename_description') }}</ShadDialogDescription>
				</ShadDialogHeader>
				<div class="flex items-center rounded-md border border-input bg-background focus-within:ring-1 focus-within:ring-ring">
					<span class="pl-3 font-mono text-xs text-muted-foreground">{{ PRESET_KEY_PREFIX }}</span>
					<input v-model="newSlug" data-testid="admin-preset-slug" class="h-9 min-w-0 flex-1 bg-transparent pr-3 font-mono text-xs outline-none" maxlength="64" spellcheck="false" autocomplete="off" />
				</div>
				<p v-if="newSlug && !isValidPresetSlug(newSlug)" class="text-xs text-destructive">{{ store.getTranslation('presets.editor.slug_invalid') }}</p>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="renaming = null">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton data-testid="admin-preset-rename-save" :disabled="!isValidPresetSlug(newSlug) || newSlug === renaming?.slug" @click="rename">{{ store.getTranslation('common.save') }}</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</SettingsPage>
</template>

<script setup lang="ts">
import {Loader2, MoreHorizontal, Pencil, Search, TextCursorInput, Wand2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {ADMIN_PRESETS_EDIT} from '~/types/generated/permissions';
import type {AdminPresetResponse, AdminUpdatePresetRequest, PresetScope, PresetTeam} from '~/types/presets';

const SCOPES: PresetScope[] = ['personal', 'team', 'installation'];

const store = useMainStore();
const router = useRouter();
const {$customFetch} = useNuxtApp();

const t = (key: string) => store.getTranslation(`presets.admin.${key}`);

const presets = ref<AdminPresetResponse[]>([]);
const loading = ref(true);
const search = ref('');
const scope = ref<'all' | PresetScope>('all');
const teamId = ref('all');
const renaming = ref<AdminPresetResponse | null>(null);
const newSlug = ref('');

const canEdit = computed(() => store.hasPermission(ADMIN_PRESETS_EDIT));

const teams = computed<PresetTeam[]>(() => {
	const byId = new Map<string, PresetTeam>();
	for (const preset of presets.value) if (preset.team) byId.set(preset.team.id, preset.team);
	return [...byId.values()].sort((a, b) => a.name.localeCompare(b.name));
});

const filtered = computed(() => {
	const query = search.value.trim().toLowerCase();
	return presets.value.filter(
		preset =>
			(scope.value === 'all' || preset.scope === scope.value) &&
			(teamId.value === 'all' || preset.team?.id === teamId.value) &&
			(!query || preset.name.toLowerCase().includes(query) || preset.key.includes(query)),
	);
});

function ownerLabel(preset: AdminPresetResponse): string {
	if (preset.scope === 'personal' && preset.owner) return preset.owner.username;
	return presetScopeLabel(preset, store.getTranslation);
}

async function load() {
	loading.value = true;
	try {
		presets.value = await $customFetch<AdminPresetResponse[]>('/api/v1/admin/presets');
	} catch (e: any) {
		store.toast(t('load_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		loading.value = false;
	}
}

async function save(preset: AdminPresetResponse, body: AdminUpdatePresetRequest): Promise<boolean> {
	try {
		const updated = await $customFetch<AdminPresetResponse>(`/api/v1/admin/presets/${preset.id}`, {method: 'PATCH', body});
		presets.value = presets.value.map(row => (row.id === updated.id ? updated : row));
		store.toast(t('saved'), {type: 'success'});
		return true;
	} catch (e: any) {
		store.toast(store.getTranslation('presets.settings.save_error'), {type: 'error', description: errorMessage(e, '')});
		return false;
	}
}

function openRename(preset: AdminPresetResponse) {
	renaming.value = preset;
	newSlug.value = preset.slug;
}

async function rename() {
	const preset = renaming.value;
	if (preset && (await save(preset, {slug: newSlug.value.trim()}))) renaming.value = null;
}

onMounted(load);
</script>
