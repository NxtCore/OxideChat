<template>
	<div class="space-y-5">
		<div>
			<h3 class="mb-1 text-base font-semibold text-foreground">{{ t('levels_title') }}</h3>
			<p class="text-xs text-muted-foreground">{{ t('levels_description') }}</p>
		</div>

		<div v-if="form.levels.length === 0" class="rounded-lg border border-dashed border-border p-5 text-center" data-testid="preset-levels-empty">
			<Gauge class="mx-auto h-6 w-6 text-muted-foreground" />
			<p class="mt-2 text-sm text-muted-foreground">{{ t('levels_empty') }}</p>
			<ShadButton variant="outline" class="mt-3" type="button" :disabled="disabled" data-testid="preset-levels-enable" @click="enable">
				<Plus class="h-4 w-4" />
				{{ t('levels_enable') }}
			</ShadButton>
		</div>

		<template v-else>
			<div role="radiogroup" :aria-label="t('level_default')" class="space-y-3">
				<div v-for="(level, index) in form.levels" :key="level.uid" class="space-y-3 rounded-lg border border-border p-3" data-testid="preset-level" :data-level="level.key">
					<div class="flex items-center gap-2">
						<span class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">{{ index + 1 }}</span>
						<span class="min-w-0 flex-1 truncate text-sm font-medium">{{ presetLevelLabel(level, store.getTranslation) || t('level_untitled') }}</span>
						<button
							type="button"
							role="radio"
							:aria-checked="form.default_level === level.key"
							:disabled="disabled"
							data-testid="preset-level-default"
							class="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs hover:bg-muted"
							:class="form.default_level === level.key ? 'text-primary' : 'text-muted-foreground'"
							@click="form.default_level = level.key"
						>
							<CircleDot v-if="form.default_level === level.key" class="h-3.5 w-3.5" />
							<Circle v-else class="h-3.5 w-3.5" />
							{{ t('level_default') }}
						</button>
						<ShadButton variant="ghost" size="icon" type="button" class="h-7 w-7" :disabled="disabled || index === 0" :aria-label="t('level_up')" @click="move(index, -1)">
							<ArrowUp class="h-3.5 w-3.5" />
						</ShadButton>
						<ShadButton variant="ghost" size="icon" type="button" class="h-7 w-7" :disabled="disabled || index === form.levels.length - 1" :aria-label="t('level_down')" @click="move(index, 1)">
							<ArrowDown class="h-3.5 w-3.5" />
						</ShadButton>
						<ShadButton
							variant="ghost"
							size="icon"
							type="button"
							class="h-7 w-7"
							:disabled="disabled || form.levels.length <= MIN_PRESET_LEVELS"
							:aria-label="t('level_remove')"
							data-testid="preset-level-remove"
							@click="remove(index)"
						>
							<Trash2 class="h-3.5 w-3.5" />
						</ShadButton>
					</div>

					<div class="grid gap-3 sm:grid-cols-2">
						<div class="space-y-1.5" data-testid="preset-level-model">
							<ShadLabel>{{ t('level_model') }}</ShadLabel>
							<SettingsDefaultModelPicker
								:model-value="level.model_key"
								:placeholder="t('choose_model')"
								:disabled="disabled"
								@update:model-value="value => (level.model_key = value)"
								@selected="model => selectModel(level, model)"
							/>
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ t('level_reasoning') }}</ShadLabel>
							<ShadSelect :model-value="level.reasoning.effort ?? INHERIT" :disabled="disabled" @update:model-value="value => (level.reasoning.effort = value === INHERIT ? null : String(value))">
								<ShadSelectTrigger class="h-9 w-full text-xs" data-testid="preset-level-reasoning">
									<ShadSelectValue />
								</ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem :value="INHERIT" class="text-xs">{{ t('level_reasoning_inherit') }}</ShadSelectItem>
									<ShadSelectItem v-for="effort in EFFORTS" :key="effort" :value="effort" class="text-xs">{{ effortLabel(effort) }}</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ t('level_label') }}</ShadLabel>
							<ShadInput v-model="level.label" maxlength="40" data-testid="preset-level-label" :placeholder="builtinLabel(level.key) ?? t('level_label_placeholder')" />
						</div>
						<div class="space-y-1.5">
							<ShadLabel>{{ t('level_key') }}</ShadLabel>
							<div class="flex items-center rounded-md border border-input bg-background focus-within:ring-1 focus-within:ring-ring">
								<span class="max-w-32 truncate pl-3 font-mono text-xs text-muted-foreground">{{ presetKey(slug || '…') }}:</span>
								<input
									:value="level.key"
									data-testid="preset-level-key"
									class="h-9 min-w-0 flex-1 bg-transparent pr-3 font-mono text-xs outline-none"
									maxlength="32"
									spellcheck="false"
									autocomplete="off"
									:disabled="disabled"
									@input="event => setKey(level, (event.target as HTMLInputElement).value)"
								/>
							</div>
							<p v-if="!isValidLevelKey(level.key)" class="text-xs text-destructive">{{ t('level_key_invalid') }}</p>
							<p v-else-if="isDuplicate(level)" class="text-xs text-destructive">{{ t('level_key_taken') }}</p>
						</div>
						<div class="space-y-1.5 sm:col-span-2">
							<ShadLabel>{{ t('level_hint') }}</ShadLabel>
							<ShadInput v-model="level.hint" maxlength="100" data-testid="preset-level-hint" :placeholder="t('level_hint_placeholder')" />
						</div>
					</div>
				</div>
			</div>

			<div class="flex flex-wrap items-center gap-2">
				<ShadButton variant="outline" type="button" :disabled="disabled || form.levels.length >= MAX_PRESET_LEVELS" data-testid="preset-levels-add" @click="add">
					<Plus class="h-4 w-4" />
					{{ t('level_add') }}
				</ShadButton>
				<ShadButton variant="ghost" type="button" class="text-muted-foreground" :disabled="disabled" data-testid="preset-levels-disable" @click="disable">
					{{ t('levels_disable') }}
				</ShadButton>
			</div>
			<p class="text-xs text-muted-foreground">{{ t('levels_note') }}</p>
		</template>
	</div>
</template>

<script setup lang="ts">
import {ArrowDown, ArrowUp, Circle, CircleDot, Gauge, Plus, Trash2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {ModelList} from '~/types/chat';
import type {LevelRow} from '~/utils/presets';

const props = defineProps<{
	form: {levels: LevelRow[]; default_level: string | null};
	slug: string;
	baseModel: ModelList | null;
	disabled?: boolean;
}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`presets.editor.${key}`);

const INHERIT = '__preset__';
const EFFORTS = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'] as const;

function effortLabel(effort: string) {
	return store.getTranslation(`chat.reasoning_selector.${effort === 'xhigh' ? 'extra_high' : effort}`);
}

function builtinLabel(key: string): string | null {
	return (BUILTIN_LEVEL_KEYS as readonly string[]).includes(key) ? store.getTranslation(`presets.level.${key}`) : null;
}

function isDuplicate(level: LevelRow) {
	return props.form.levels.some(other => other !== level && other.key === level.key);
}

function unusedKey(): string {
	const taken = new Set(props.form.levels.map(level => level.key));
	return BUILTIN_LEVEL_KEYS.find(key => !taken.has(key)) ?? Array.from({length: MAX_PRESET_LEVELS + 1}, (_, n) => `level-${n + 1}`).find(key => !taken.has(key))!;
}

function enable() {
	props.form.levels.push(levelRow('low', props.baseModel), levelRow('high', props.baseModel));
	props.form.default_level = 'low';
}

function disable() {
	props.form.levels.splice(0);
	props.form.default_level = null;
}

function add() {
	props.form.levels.push(levelRow(unusedKey(), null));
}

function remove(index: number) {
	const [removed] = props.form.levels.splice(index, 1);
	if (removed && props.form.default_level === removed.key) props.form.default_level = props.form.levels[0]?.key ?? null;
}

function move(index: number, step: number) {
	const target = index + step;
	if (target < 0 || target >= props.form.levels.length) return;
	const [level] = props.form.levels.splice(index, 1);
	if (level) props.form.levels.splice(target, 0, level);
}

function setKey(level: LevelRow, key: string) {
	if (props.form.default_level === level.key && !isDuplicate(level)) props.form.default_level = key;
	level.key = key;
}

function selectModel(level: LevelRow, model: ModelList | null) {
	level.model_id = model?.id ?? '';
	level.model_key = model?.model_id ?? null;
	level.model_name = model?.display_name ?? null;
}
</script>
