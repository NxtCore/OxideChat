<template>
	<SettingsPage :title="t('title')" :description="t('description')">
		<template #actions>
			<ShadButton data-testid="preset-create" @click="router.push('/settings/presets/new')">
				<Plus class="h-4 w-4" />
				{{ t('create') }}
			</ShadButton>
		</template>

		<div v-if="!presetStore.loaded" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="own.length === 0 && shared.length === 0" class="flex flex-col items-center rounded-xl border border-dashed border-border px-6 py-12 text-center" data-testid="presets-empty">
			<Wand2 class="mb-4 h-10 w-10 text-primary" />
			<p class="font-medium text-foreground">{{ t('empty_title') }}</p>
			<p class="mt-1 max-w-md text-sm text-muted-foreground">{{ t('empty_description') }}</p>
			<div class="mt-5 flex flex-wrap justify-center gap-2">
				<ShadButton @click="router.push('/settings/presets/new')">
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

		<div v-else class="space-y-6">
			<section v-for="group in groups" :key="group.key" class="space-y-2">
				<p v-if="shared.length > 0" class="text-xs font-medium text-muted-foreground">{{ t(group.label) }}</p>
				<div
					v-for="preset in group.items"
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
						:aria-label="t('enabled')"
						@update:model-value="value => setEnabled(preset, Boolean(value))"
					/>
					<ShadDropdownMenu v-if="preset.can_edit">
						<ShadDropdownMenuTrigger as-child>
							<ShadButton variant="ghost" size="icon" class="h-8 w-8" data-testid="preset-row-menu" :aria-label="t('actions')">
								<MoreHorizontal class="h-4 w-4" />
							</ShadButton>
						</ShadDropdownMenuTrigger>
						<ShadDropdownMenuContent align="end" class="border-border bg-popover">
							<ShadDropdownMenuItem class="text-popover-foreground focus:bg-accent" @click="router.push(`/settings/presets/${preset.id}`)">
								<Pencil class="mr-2 h-4 w-4" />
								{{ t('edit') }}
							</ShadDropdownMenuItem>
							<ShadDropdownMenuItem class="text-popover-foreground focus:bg-accent" @click="duplicate(preset)">
								<CopyPlus class="mr-2 h-4 w-4" />
								{{ t('duplicate') }}
							</ShadDropdownMenuItem>
							<ShadDropdownMenuSeparator class="bg-border" />
							<ShadDropdownMenuItem data-testid="preset-row-delete" class="text-destructive focus:bg-destructive/10" @click="pendingDelete = preset">
								<Trash2 class="mr-2 h-4 w-4" />
								{{ t('delete') }}
							</ShadDropdownMenuItem>
						</ShadDropdownMenuContent>
					</ShadDropdownMenu>
					<ChevronRight v-else class="h-5 w-5 text-muted-foreground" />
				</div>
			</section>
		</div>

		<SettingsPresetsDeleteDialog :preset="pendingDelete" @close="pendingDelete = null" />
	</SettingsPage>
</template>

<script setup lang="ts">
import {ChevronRight, Copy, CopyPlus, Loader2, MessageSquare, MoreHorizontal, Pencil, Plus, Trash2, Wand2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import {usePresetStore} from '~/stores/presetStore';
import type {PresetResponse} from '~/types/presets';

const store = useMainStore();
const presetStore = usePresetStore();
const router = useRouter();

const t = (key: string) => store.getTranslation(`presets.settings.${key}`);

const pendingDelete = ref<PresetResponse | null>(null);

const own = computed(() => presetStore.presets.filter(preset => preset.can_edit));
const shared = computed(() => presetStore.presets.filter(preset => !preset.can_edit));
const groups = computed(() =>
	[
		{key: 'own', label: 'mine', items: own.value},
		{key: 'shared', label: 'shared', items: shared.value},
	].filter(group => group.items.length > 0),
);

onMounted(() => presetStore.ensurePresets());

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
