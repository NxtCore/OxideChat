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

		toggle(id: string) {
			this.selected = this.selected.includes(id) ? this.selected.filter(selected => selected !== id) : [...this.selected, id];
		},
	},
});
