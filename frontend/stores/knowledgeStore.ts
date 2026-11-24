import {defineStore} from 'pinia';
import type {KnowledgeBaseResponse} from '~/types/generated/KnowledgeBaseResponse';

export const useKnowledgeStore = defineStore('knowledge', {
	state: () => ({
		bases: [] as KnowledgeBaseResponse[],
		loaded: false,
		stale: false,
		/** Knowledge bases the composer attaches to the chat with the next turn. */
		selected: [] as string[],
	}),

	getters: {
		usable: state => state.bases.filter(base => base.usable),
	},

	actions: {
		async ensure() {
			if (this.loaded && !this.stale) return;
			await inflight('knowledge-bases', async () => {
				try {
					this.bases = await knowledgeBases();
					this.loaded = true;
					this.stale = false;
				} catch (e) {
					console.error('Failed to load knowledge bases:', e);
				}
			});
		},

		/** Whether a knowledge base's model access lets it reach `modelId`; the server checks this again. */
		allowsModel(base: KnowledgeBaseResponse, modelId: string | null) {
			return !base.allowed_model_ids || !modelId || base.allowed_model_ids.includes(modelId);
		},

		/** The selected knowledge bases that may reach `modelId`. */
		selectedFor(modelId: string | null) {
			return this.selected.filter(id => {
				const base = this.bases.find(candidate => candidate.id === id);
				return !base || this.allowsModel(base, modelId);
			});
		},

		toggle(id: string) {
			this.selected = this.selected.includes(id) ? this.selected.filter(selected => selected !== id) : [...this.selected, id];
		},
	},
});
