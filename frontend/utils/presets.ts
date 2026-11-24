import type {ModelList} from '~/types/chat';
import type {ComposerModel, PresetLevel, PresetLevelResponse, PresetResponse, PresetScope, PresetTeam, SavePresetRequest} from '~/types/presets';

export const PRESET_KEY_PREFIX = 'presets/';

export const MAX_PRESET_STARTERS = 4;

export const PRESET_LEVEL_SEPARATOR = ':';

export const MIN_PRESET_LEVELS = 2;

export const MAX_PRESET_LEVELS = 5;

export const BUILTIN_LEVEL_KEYS = ['low', 'medium', 'high', 'max'] as const;

export const presetsPath = '/api/v1/presets';

const SLUG_PART = '[a-z0-9]([a-z0-9-]*[a-z0-9])?';
const LEVEL_KEY_PATTERN = new RegExp(`^${SLUG_PART}$`);
const PRESET_SLUG_PATTERN = new RegExp(`^${SLUG_PART}(/${SLUG_PART})*$`);

export function isPresetKey(key: string | null | undefined): boolean {
	return !!key && key.startsWith(PRESET_KEY_PREFIX);
}

export function presetKey(slug: string, level?: string | null): string {
	return level ? `${PRESET_KEY_PREFIX}${slug}${PRESET_LEVEL_SEPARATOR}${level}` : `${PRESET_KEY_PREFIX}${slug}`;
}

export function splitPresetKey(key: string): {key: string; level: string | null} {
	const index = key.indexOf(PRESET_LEVEL_SEPARATOR, PRESET_KEY_PREFIX.length);
	return index === -1 ? {key, level: null} : {key: key.slice(0, index), level: key.slice(index + 1) || null};
}

export function isValidLevelKey(key: string): boolean {
	return key.length > 0 && key.length <= 32 && LEVEL_KEY_PATTERN.test(key);
}

export function presetLevelLabel(level: {key: string; label?: string | null}, translate: (key: string) => string): string {
	if (level.label) return level.label;
	return (BUILTIN_LEVEL_KEYS as readonly string[]).includes(level.key) ? translate(`presets.level.${level.key}`) : level.key;
}

export type LevelRow = {
	uid: number;
	key: string;
	label: string;
	hint: string;
	model_id: string;
	model_key: string | null;
	model_name: string | null;
	reasoning: {effort: string | null; budget_tokens: number | null};
};

let nextLevelUid = 1;

export function levelRow(key: string, model: Pick<ModelList, 'id' | 'model_id' | 'display_name'> | null): LevelRow {
	return {
		uid: nextLevelUid++,
		key,
		label: '',
		hint: '',
		model_id: model?.id ?? '',
		model_key: model?.model_id ?? null,
		model_name: model?.display_name ?? null,
		reasoning: {effort: null, budget_tokens: null},
	};
}

export function hasLevels(preset: PresetResponse | null | undefined): boolean {
	return !!preset && preset.levels.length > 0;
}

export function isLevelAvailable(level: PresetLevelResponse | undefined): level is PresetLevelResponse {
	return !!level && !level.unavailable && !!level.model;
}

export function isLevelUsable(level: PresetLevelResponse | undefined): level is PresetLevelResponse {
	return isLevelAvailable(level) && !level.model?.budget_blocked;
}

export function usableLevel(preset: PresetResponse, levelKey?: string | null): PresetLevelResponse | null {
	const candidates = [preset.levels.find(level => level.key === levelKey), preset.levels.find(level => level.key === preset.default_level), ...preset.levels];
	return candidates.find(isLevelUsable) ?? candidates.find(isLevelAvailable) ?? null;
}

export function levelToRequest(level: PresetLevelResponse): PresetLevel {
	return {key: level.key, label: level.label, hint: level.hint, model_id: level.model_id, reasoning: {...level.reasoning}};
}

function slugifyPart(part: string): string {
	return part
		.normalize('NFKD')
		.replace(/\p{M}/gu, '')
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');
}

export function slugify(name: string): string {
	return name
		.split('/')
		.map(slugifyPart)
		.filter(Boolean)
		.join('/')
		.slice(0, 64)
		.replace(/[-/]+$/g, '');
}

export function isValidPresetSlug(slug: string): boolean {
	return slug.length > 0 && slug.length <= 64 && PRESET_SLUG_PATTERN.test(slug);
}

export function isArena(preset: PresetResponse | null | undefined): boolean {
	return preset?.mode === 'arena';
}

export function presetAsModel(preset: PresetResponse, levelKey?: string | null): ComposerModel | null {
	if (isArena(preset)) {
		const first = preset.levels.find(level => level.model)?.model;
		if (!first) return null;
		return {...first, id: preset.id, model_id: preset.key, display_name: preset.name, icon: preset.icon ?? null, is_favorite: false, preset};
	}
	if (hasLevels(preset)) {
		const level = usableLevel(preset, levelKey);
		if (!level?.model) return null;
		return {
			...level.model,
			id: preset.id,
			model_id: presetKey(preset.slug, level.key),
			display_name: preset.name,
			icon: preset.icon ?? level.model.icon,
			is_favorite: false,
			preset,
			level,
		};
	}
	const base = preset.base_model;
	if (!base) return null;
	return {
		...base,
		id: preset.id,
		model_id: preset.key,
		display_name: preset.name,
		icon: preset.icon ?? base.icon,
		is_favorite: false,
		preset,
	};
}

export function baseModelId(model: ComposerModel | ModelList): string {
	const composer = model as ComposerModel;
	return composer.level?.model_id ?? composer.preset?.base_model_id ?? model.id;
}

export function presetToRequest(preset: PresetResponse): SavePresetRequest {
	return {
		slug: preset.slug,
		name: preset.name,
		description: preset.description,
		icon: preset.icon,
		base_model_id: preset.base_model_id ?? '',
		system_prompt: preset.system_prompt,
		sampling: {...preset.sampling},
		reasoning: {...preset.reasoning},
		levels: preset.levels.map(levelToRequest),
		default_level: preset.default_level,
		tool_ids: [...preset.tool_ids],
		knowledge_base_ids: [...preset.knowledge_base_ids],
		tools_locked: preset.tools_locked,
		starters: [...preset.starters],
		prompt_visible: preset.prompt_visible,
		is_enabled: preset.is_enabled,
		arena: {...preset.arena},
	};
}

export function presetScopeLabel(preset: {scope: PresetScope; team: PresetTeam | null; shared?: boolean}, translate: (key: string) => string): string {
	if (preset.shared) return translate('presets.scope.shared');
	return preset.scope === 'team' && preset.team ? preset.team.name : translate(`presets.scope.${preset.scope}`);
}
