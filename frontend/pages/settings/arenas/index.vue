<template>
	<SettingsPage :title="t('title')" :description="t('description')">
		<template v-if="canCreate" #actions>
			<ShadButton data-testid="arena-create" @click="router.push('/settings/arenas/new')">
				<Plus class="h-4 w-4" />
				{{ t('create') }}
			</ShadButton>
		</template>

		<div v-if="!presetStore.loaded" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="arenas.length === 0" class="flex flex-col items-center rounded-xl border border-dashed border-border px-6 py-12 text-center" data-testid="arenas-empty">
			<Swords class="mb-4 h-10 w-10 text-primary" />
			<p class="font-medium text-foreground">{{ t('empty_title') }}</p>
			<p class="mt-1 max-w-md text-sm text-muted-foreground">{{ t('empty_description') }}</p>
		</div>

		<div v-else class="space-y-2">
			<div v-for="arena in arenas" :key="arena.id" data-testid="arena-row" :data-preset-key="arena.key" class="flex items-center gap-4 rounded-lg border border-border bg-card px-4 py-3">
				<div class="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-md bg-primary/10">
					<img v-if="arena.icon" :src="arena.icon" alt="" class="h-full w-full object-cover" />
					<Swords v-else class="h-5 w-5 text-primary" />
				</div>
				<NuxtLink :to="`/settings/arenas/${arena.id}`" class="min-w-0 flex-1">
					<div class="flex flex-wrap items-center gap-2">
						<span class="font-medium text-foreground">{{ arena.name }}</span>
						<span class="rounded bg-primary/10 px-1.5 py-0.5 text-xs text-primary">{{ presetScopeLabel(arena, store.getTranslation) }}</span>
						<span v-if="arena.unavailable" class="rounded-full bg-destructive/10 px-2 py-0.5 text-xs text-destructive">
							{{ store.getTranslation(`presets.unavailable.${arena.unavailable}`) }}
						</span>
					</div>
					<p class="truncate text-sm text-muted-foreground">{{ store.getTranslation('presets.arena.picker_subtitle', {count: String(arena.levels.length)}) }}</p>
				</NuxtLink>
				<ShadSwitch
					v-if="arena.can_edit"
					data-testid="arena-row-enabled"
					:model-value="arena.is_enabled"
					:disabled="arena.disabled_by_admin && !presetStore.scopes?.installation"
					:aria-label="store.getTranslation('presets.settings.enabled')"
					@update:model-value="value => setEnabled(arena, Boolean(value))"
				/>
			</div>
		</div>
	</SettingsPage>
</template>

<script setup lang="ts">
import {Loader2, Plus, Swords} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {usePresetStore} from '~/stores/presetStore';
import type {PresetResponse} from '~/types/presets';

const store = useMainStore();
const presetStore = usePresetStore();
const router = useRouter();

const t = (key: string) => store.getTranslation(`presets.arena.settings.${key}`);

const arenas = computed(() => presetStore.presets.filter(isArena));
const canCreate = computed(() => !!presetStore.scopes && (presetStore.scopes.installation || presetStore.scopes.teams.length > 0));

onMounted(() => Promise.all([presetStore.ensurePresets(), presetStore.ensureScopes()]));

async function setEnabled(arena: PresetResponse, enabled: boolean) {
	try {
		await presetStore.updatePreset(arena.id, {...presetToRequest(arena), is_enabled: enabled});
		store.toast(store.getTranslation(enabled ? 'presets.settings.enabled_toast' : 'presets.settings.disabled_toast'), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('presets.settings.save_error'), {type: 'error', description: errorMessage(e, '')});
	}
}
</script>
