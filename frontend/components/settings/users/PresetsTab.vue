<template>
	<SettingsUsersSection
		:title="t('presets_title')"
		:description="t('presets_description')"
		to="/settings/presets?tab=all"
		:link-label="t('manage_presets')"
		:empty-title="presets.length ? undefined : t('presets_empty')"
		:empty-hint="t('presets_empty_hint')"
		test-id="user-presets"
	>
		<ul class="divide-y divide-border rounded-lg border border-border bg-card">
			<li v-for="preset in presets" :key="preset.id" class="flex items-center gap-4 px-4 py-3" data-testid="user-preset" :data-preset-key="preset.key">
				<span class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-md bg-primary/10">
					<img v-if="preset.icon" :src="preset.icon" alt="" class="h-full w-full object-cover" />
					<Wand2 v-else class="h-4 w-4 text-primary" />
				</span>
				<div class="min-w-0 flex-1">
					<div class="flex flex-wrap items-center gap-2">
						<span class="truncate text-sm font-medium text-foreground">{{ preset.name }}</span>
						<span class="rounded bg-primary/10 px-1.5 py-0.5 text-xs text-primary" data-testid="user-preset-source">{{ sourceOf(preset) }}</span>
						<span v-if="!preset.is_enabled" class="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">{{ t('preset_off') }}</span>
					</div>
					<p class="truncate font-mono text-xs text-muted-foreground">
						{{ preset.key }}<template v-if="preset.base_model_name"> · {{ preset.base_model_name }}</template>
					</p>
				</div>
			</li>
		</ul>
	</SettingsUsersSection>
</template>

<script setup lang="ts">
import {Wand2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AdminPresetResponse} from '~/types/presets';
import type {UserTeamResponse} from '~/types/users';

const props = defineProps<{presets: AdminPresetResponse[]; userId: string; teams: UserTeamResponse[]}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`settings.user_detail.${key}`);

function sourceOf(preset: AdminPresetResponse): string {
	const own = preset.scope === 'personal' && preset.owner?.id === props.userId;
	const team = preset.scope === 'team' && props.teams.some(member => member.id === preset.team?.id);
	return presetScopeLabel({...preset, shared: !own && !team && preset.scope !== 'installation'}, store.getTranslation);
}
</script>
