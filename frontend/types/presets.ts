import type {ModelList} from './generated/ModelList';
import type {PresetResponse} from './generated/PresetResponse';
import type {SavePresetRequest} from './generated/SavePresetRequest';

export type {PresetResponse} from './generated/PresetResponse';
export type {PresetSampling} from './generated/PresetSampling';
export type {PresetReasoning} from './generated/PresetReasoning';
export type {PresetScope} from './generated/PresetScope';
export type {PresetUnavailable} from './generated/PresetUnavailable';
export type {PresetSlugAvailability} from './generated/PresetSlugAvailability';
export type {SavePresetRequest} from './generated/SavePresetRequest';
export type {PresetTeam} from './generated/PresetTeam';
export type {PresetScopesResponse} from './generated/PresetScopesResponse';
export type {AdminPresetResponse} from './generated/AdminPresetResponse';
export type {AdminUpdatePresetRequest} from './generated/AdminUpdatePresetRequest';

export type ComposerModel = ModelList & {preset?: PresetResponse};

export type PresetDraft = SavePresetRequest & {base_model?: ModelList | null};
