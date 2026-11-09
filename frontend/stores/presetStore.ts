import {defineStore} from 'pinia';
import type {ComposerModel, PresetDraft, PresetResponse, PresetSlugAvailability, SavePresetRequest} from '~/types/presets';

export const usePresetStore = defineStore('preset', {
	state: () => ({
		presets: [] as PresetResponse[],
		loaded: false,
		stale: false,
		draft: null as PresetDraft | null,
	}),

	getters: {
		byKey: state => (key: string) => state.presets.find(preset => preset.key === key) ?? null,
		usable: state => state.presets.filter(preset => !preset.unavailable && preset.base_model),
	},

	actions: {
		async ensurePresets() {
			if (this.loaded && !this.stale) return;
			await inflight('presets', async () => {
				try {
					const {$customFetch} = useNuxtApp();
					this.presets = await $customFetch<PresetResponse[]>(presetsPath);
					this.loaded = true;
					this.stale = false;
				} catch (e) {
					console.error('Failed to load presets:', e);
				}
			});
		},

		async modelForKey(key: string): Promise<ComposerModel | null> {
			await this.ensurePresets();
			const preset = this.byKey(key);
			return preset && !preset.unavailable ? presetAsModel(preset) : null;
		},

		async fetchPreset(id: string): Promise<PresetResponse> {
			const {$customFetch} = useNuxtApp();
			return await $customFetch<PresetResponse>(`${presetsPath}/${id}`);
		},

		async createPreset(body: SavePresetRequest): Promise<PresetResponse> {
			const {$customFetch} = useNuxtApp();
			const created = await $customFetch<PresetResponse>(presetsPath, {method: 'POST', body});
			await this.ensurePresets();
			return created;
		},

		async updatePreset(id: string, body: SavePresetRequest): Promise<PresetResponse> {
			const {$customFetch} = useNuxtApp();
			const updated = await $customFetch<PresetResponse>(`${presetsPath}/${id}`, {method: 'PUT', body});
			await this.ensurePresets();
			return updated;
		},

		async deletePreset(id: string) {
			const {$customFetch} = useNuxtApp();
			await $customFetch(`${presetsPath}/${id}`, {method: 'DELETE'});
			await this.ensurePresets();
		},

		async checkSlug(slug: string, excludeId?: string): Promise<PresetSlugAvailability> {
			const {$customFetch} = useNuxtApp();
			const params: Record<string, string> = {slug};
			if (excludeId) params.exclude_id = excludeId;
			return await $customFetch<PresetSlugAvailability>(`${presetsPath}/check-slug`, {params});
		},
	},
});
