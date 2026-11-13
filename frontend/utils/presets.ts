import type {ModelList} from '~/types/chat';
import type {ComposerModel, PresetResponse, PresetScope, PresetTeam, SavePresetRequest} from '~/types/presets';

export const PRESET_KEY_PREFIX = 'presets/';

export const MAX_PRESET_STARTERS = 4;

export const presetsPath = '/api/v1/presets';

export function isPresetKey(key: string | null | undefined): boolean {
	return !!key && key.startsWith(PRESET_KEY_PREFIX);
}

export function presetKey(slug: string): string {
	return `${PRESET_KEY_PREFIX}${slug}`;
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
	return slug.length > 0 && slug.length <= 64 && /^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\/[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$/.test(slug);
}

export function presetAsModel(preset: PresetResponse): ComposerModel | null {
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
	return (model as ComposerModel).preset?.base_model_id ?? model.id;
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
		tool_ids: [...preset.tool_ids],
		tools_locked: preset.tools_locked,
		starters: [...preset.starters],
		prompt_visible: preset.prompt_visible,
		is_enabled: preset.is_enabled,
	};
}

export function presetScopeLabel(preset: {scope: PresetScope; team: PresetTeam | null; shared?: boolean}, translate: (key: string) => string): string {
	if (preset.shared) return translate('presets.scope.shared');
	return preset.scope === 'team' && preset.team ? preset.team.name : translate(`presets.scope.${preset.scope}`);
}
