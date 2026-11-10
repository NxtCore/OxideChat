<template>
	<div class="space-y-5">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">{{ t('general_title') }}</h3>
			<p class="text-xs text-muted-foreground">{{ t('general_description') }}</p>
		</div>

		<div class="space-y-4">
			<div class="space-y-1.5">
				<ShadLabel for="preset-name">{{ t('name') }}</ShadLabel>
				<ShadInput
					id="preset-name"
					:model-value="form.name"
					data-testid="preset-name"
					:placeholder="t('name_placeholder')"
					maxlength="100"
					@update:model-value="value => emit('name', String(value))"
				/>
			</div>

			<div class="space-y-1.5">
				<ShadLabel for="preset-slug">{{ t('slug') }}</ShadLabel>
				<div class="flex items-center gap-2">
					<div class="flex flex-1 items-center rounded-md border border-input bg-background focus-within:ring-1 focus-within:ring-ring">
						<span class="pl-3 font-mono text-xs text-muted-foreground">{{ PRESET_KEY_PREFIX }}</span>
						<input
							id="preset-slug"
							:value="form.slug"
							data-testid="preset-slug"
							class="h-9 min-w-0 flex-1 bg-transparent pr-3 font-mono text-xs outline-none"
							maxlength="64"
							spellcheck="false"
							autocomplete="off"
							@input="event => emit('slug', (event.target as HTMLInputElement).value)"
						/>
						<span class="pr-3" data-testid="preset-slug-status" :data-status="slugStatus">
							<Loader2 v-if="slugStatus === 'checking'" class="h-4 w-4 animate-spin text-muted-foreground" />
							<Check v-else-if="slugStatus === 'available'" class="h-4 w-4 text-green-600" />
							<X v-else-if="slugStatus === 'taken' || slugStatus === 'invalid'" class="h-4 w-4 text-destructive" />
						</span>
					</div>
					<ShadButton variant="outline" size="icon" type="button" class="flex-shrink-0" :aria-label="store.getTranslation('presets.settings.copy_id')" @click="store.copyToClipboard(presetKey(form.slug))">
						<Copy class="h-3.5 w-3.5" />
					</ShadButton>
				</div>
				<p class="text-xs" :class="slugStatus === 'taken' || slugStatus === 'invalid' ? 'text-destructive' : 'text-muted-foreground'">{{ slugHint }}</p>
				<p v-if="slugRenamed" class="flex items-start gap-1.5 text-xs text-amber-600 dark:text-amber-500" data-testid="preset-slug-renamed">
					<AlertTriangle class="mt-0.5 h-3.5 w-3.5 shrink-0" />
					{{ t('slug_renamed') }}
				</p>
				<p class="text-xs text-muted-foreground">
					{{ t('api_hint') }}
					<NuxtLink to="/developer/models" class="text-primary hover:underline">{{ t('api_link') }}</NuxtLink>
				</p>
			</div>

			<div class="space-y-1.5" data-testid="preset-base-model">
				<ShadLabel>{{ t('model_title') }}</ShadLabel>
				<SettingsDefaultModelPicker :model-value="baseModelKey" :placeholder="t('choose_model')" :disabled="disabled" @update:model-value="value => emit('base-key', value)" @selected="model => emit('base', model)" />
				<p class="text-xs text-muted-foreground">{{ t('model_description') }}</p>
			</div>

			<div class="space-y-1.5">
				<ShadLabel for="preset-description">{{ t('description') }}</ShadLabel>
				<ShadTextarea id="preset-description" v-model="form.description" :placeholder="t('description_placeholder')" rows="3" maxlength="2000" />
				<p class="text-xs text-muted-foreground">{{ t('description_hint') }}</p>
			</div>

			<div class="flex items-center justify-between rounded-lg border border-border p-3">
				<div class="space-y-0.5">
					<ShadLabel>{{ t('enabled') }}</ShadLabel>
					<p class="text-xs text-muted-foreground">{{ t('enabled_description') }}</p>
				</div>
				<ShadSwitch v-model:model-value="form.is_enabled" />
			</div>

			<div class="space-y-1.5">
				<ShadLabel>{{ t('icon') }}</ShadLabel>
				<ImagePicker v-model="form.icon" :disabled="disabled" test-id="preset-icon">
					<template #fallback><Wand2 class="h-1/2 w-1/2 text-primary" /></template>
				</ImagePicker>
			</div>

			<div v-if="scopeOptions" class="space-y-1.5">
				<ShadLabel>{{ t('scope') }}</ShadLabel>
				<ShadSelect :model-value="scopeValue" @update:model-value="value => emit('scope', String(value))">
					<ShadSelectTrigger data-testid="preset-scope">
						<ShadSelectValue />
					</ShadSelectTrigger>
					<ShadSelectContent>
						<ShadSelectItem v-for="option in scopeOptions" :key="option.value" :value="option.value" :data-testid="`preset-scope-${option.value}`">{{ option.label }}</ShadSelectItem>
					</ShadSelectContent>
				</ShadSelect>
				<p class="text-xs text-muted-foreground">{{ t('scope_choose_description') }}</p>
			</div>
			<div v-else class="rounded-lg border border-border bg-muted/30 p-3">
				<p class="text-sm font-medium text-foreground">{{ t('scope') }}: {{ scopeLabel }}</p>
				<p class="text-xs text-muted-foreground">{{ t('scope_description') }}</p>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {AlertTriangle, Check, Copy, Loader2, Wand2, X} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {ModelList} from '~/types/chat';

export type SlugStatus = 'idle' | 'checking' | 'available' | 'taken' | 'invalid';

const props = defineProps<{
	form: Record<string, any>;
	slugStatus: SlugStatus;
	baseModelKey: string | null;
	scopeLabel: string;
	scopeOptions: {value: string; label: string}[] | null;
	scopeValue: string;
	slugRenamed: boolean;
	disabled?: boolean;
}>();

const emit = defineEmits<{
	name: [value: string];
	slug: [value: string];
	'base-key': [value: string | null];
	base: [model: ModelList | null];
	scope: [value: string];
}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`presets.editor.${key}`);

const slugHint = computed(() => {
	if (props.slugStatus === 'invalid') return t('slug_invalid');
	if (props.slugStatus === 'taken') return t('slug_taken');
	return t('slug_hint');
});
</script>
