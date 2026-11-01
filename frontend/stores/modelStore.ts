import {defineStore} from 'pinia';
import {useMainStore} from './index';
import type {ChatMessage, ModelList, PaginatedResponse} from '~/types/chat';

export const useModelStore = defineStore('model', {
	state: () => ({
		models: [] as ModelList[],
		selectedModel: null as ModelList | null,
		modelsLoading: false,
		contextTokens: 0,
		reasoningEffort: null as string | null,
		reasoningBudget: null as number | null,
		enabledTools: [] as string[],
		selectedProviderSlug: null as string | null,
		providerRoutingMode: 'prefer' as 'prefer' | 'lock',
	}),

	getters: {
		favoriteModels(): ModelList[] {
			return this.models.filter(m => m.is_favorite);
		},

		groupedModels(): Record<string, ModelList[]> {
			const grouped: Record<string, ModelList[]> = {};
			for (const model of this.models) {
				const provider = model.provider_name;
				if (!grouped[provider]) grouped[provider] = [];
				grouped[provider].push(model);
			}
			return grouped;
		},

		hasReasoningCapability: state => (model: ModelList | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.capabilities.some(c => c === 'REASONING' || c.startsWith('REASONING_'));
		},

		hasConfigurableReasoningCapability: state => (model: ModelList | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.capabilities.some(c => c.startsWith('REASONING_EFFORT_') || c.startsWith('REASONING_BUDGET_TOKENS_'));
		},

		hasToolCapability: state => (model: ModelList | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.capabilities.includes('TOOLS');
		},

		hasVisionCapability: state => (model: ModelList | null) => {
			if (!model) model = state.selectedModel;
			if (!model) return false;
			return model.input_modalities.includes('IMAGE');
		},

		availableReasoningEfforts(): string[] {
			if (!this.selectedModel) return [];
			return this.selectedModel.capabilities.filter(c => c.startsWith('REASONING_EFFORT_')).map(c => c.replace('REASONING_EFFORT_', ''));
		},

		hasNoneReasoningOption(): boolean {
			return this.availableReasoningEfforts.includes('NONE');
		},

		isReasoningRequired(): boolean {
			return this.hasReasoningCapability(null) && this.availableReasoningEfforts.length < 2;
		},

		lowestReasoningEffort(): string | null {
			const efforts = this.availableReasoningEfforts;
			if (efforts.length === 0) return null;
			const effortOrder = ['MINIMAL', 'LOW', 'MEDIUM', 'HIGH', 'XHIGH'];
			for (const effort of effortOrder) {
				if (efforts.includes(effort)) return effort;
			}
			return efforts[0] || null;
		},

		contextPercentage(): number {
			if (!this.selectedModel?.context_length) return 0;
			return Math.min(100, (this.contextTokens / this.selectedModel.context_length) * 100);
		},

		isFavoriteModel: _state => (model: ModelList) => {
			return model.is_favorite;
		},
	},

	actions: {
		async resetComposerToDefaults() {
			const prefs = useMainStore().preferences;
			const key = prefs?.effective_default_model_key;
			const model = key ? await this.findModelByKey(key) : (this.models[0] ?? null);
			this.setSelectedModel(model);
			this.selectedProviderSlug = prefs?.default_provider_slug ?? null;
			this.providerRoutingMode = 'prefer';
			this.enabledTools = prefs?.default_tools ? [...prefs.default_tools] : [];
		},

		async fetchModels() {
			this.modelsLoading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const models = await $customFetch<PaginatedResponse<ModelList>>('/api/v1/models?size=100');
				this.models = models.items;

				if (!this.selectedModel && this.models.length > 0) {
					await this.resetComposerToDefaults();
				}
			} catch (e) {
				console.error('Failed to fetch models:', e);
			} finally {
				this.modelsLoading = false;
			}
		},

		async findModelByKey(modelKey: string): Promise<ModelList | null> {
			let model = this.models.find(m => m.model_id === modelKey) ?? null;
			if (model) return model;

			if (this.models.length === 0) {
				await this.fetchModels();
				model = this.models.find(m => m.model_id === modelKey) ?? null;
				if (model) return model;
			}

			try {
				const {$customFetch} = useNuxtApp();
				const result = await $customFetch<PaginatedResponse<ModelList>>('/api/v1/models', {
					params: {
						query: modelKey,
						size: '10',
					},
				});
				const exact = result.items.find(m => m.model_id === modelKey) ?? null;
				if (exact && !this.models.some(m => m.id === exact.id)) {
					this.models.push(exact);
				}
				return exact;
			} catch (e) {
				console.error('Failed to fetch model for chat hydration:', e);
				return null;
			}
		},

		async hydrateComposerFromMessages(messages: ChatMessage[]) {
			const source = messages.findLast(m => m.role === 'assistant') ?? messages.findLast(m => m.role === 'user');
			if (!source) return;

			const settings = source.request_settings ?? {};
			const modelKey = source.model_key ?? settings.model_key ?? null;
			if (modelKey) {
				const model = await this.findModelByKey(modelKey);
				if (model && model.id !== this.selectedModel?.id) {
					this.setSelectedModel(model);
				}
			}

			this.reasoningEffort = source.reasoning_details?.effort ?? null;
			this.reasoningBudget = source.reasoning_details?.budget_tokens ?? null;
			this.selectedProviderSlug = settings.provider_slug ?? null;
			this.providerRoutingMode = settings.provider_routing_mode === 'lock' ? 'lock' : 'prefer';
			this.enabledTools = Array.isArray(settings.enabled_tools) ? [...settings.enabled_tools] : [];
		},

		setProviderSelection(slug: string | null, mode?: 'prefer' | 'lock') {
			this.selectedProviderSlug = slug;
			if (mode) this.providerRoutingMode = mode;
		},

		setSelectedModel(model: ModelList | null) {
			const modelChanged = this.selectedModel?.id !== model?.id;
			this.selectedModel = model;

			if (modelChanged) {
				const prefs = useMainStore().preferences;
				if (model && prefs?.default_model_key && model.model_id === prefs.default_model_key) {
					this.selectedProviderSlug = prefs.default_provider_slug ?? null;
				} else {
					this.selectedProviderSlug = null;
				}
			}

			if (!model) {
				this.reasoningEffort = null;
				this.reasoningBudget = null;
				return;
			}

			const hasNone = model.capabilities.some(c => c === 'REASONING_EFFORT_NONE');
			const hasReasoningCap = model.capabilities.some(c => c === 'REASONING' || c.startsWith('REASONING_'));

			if (!hasReasoningCap) {
				this.reasoningEffort = null;
				this.reasoningBudget = null;
			} else if (!hasNone) {
				const efforts = model.capabilities.filter(c => c.startsWith('REASONING_EFFORT_')).map(c => c.replace('REASONING_EFFORT_', ''));
				const effortOrder = ['MINIMAL', 'LOW', 'MEDIUM', 'HIGH', 'XHIGH'];
				const lowestEffort = effortOrder.find(e => efforts.includes(e));
				this.reasoningEffort = lowestEffort ? lowestEffort.toLowerCase() : null;
				this.reasoningBudget = null;
			} else {
				this.reasoningEffort = null;
				this.reasoningBudget = null;
			}
		},

		async toggleFavoriteModel(modelDbId: string): Promise<boolean> {
			const model = this.models.find(m => m.id === modelDbId);
			const isFavorite = !(model?.is_favorite ?? false);

			if (model) model.is_favorite = isFavorite;

			try {
				const {$customFetch} = useNuxtApp();
				await $customFetch(`/api/v1/models/${modelDbId}/favorite`, {
					method: 'POST',
					body: {is_favorite: isFavorite},
				});
				return true;
			} catch (e) {
				if (model) model.is_favorite = !isFavorite;
				console.error('Failed to toggle model favorite:', e);
				return false;
			}
		},

		setReasoningEffort(effort: string | null, isTokenBudget = false) {
			if (isTokenBudget) {
				this.reasoningBudget = effort ? parseInt(effort) : null;
			} else {
				this.reasoningEffort = effort;
			}
		},

		toggleTool(toolName: string) {
			const index = this.enabledTools.indexOf(toolName);
			if (index === -1) {
				this.enabledTools.push(toolName);
			} else {
				this.enabledTools.splice(index, 1);
			}
		},

		setContextTokens(tokens: number) {
			this.contextTokens = tokens;
		},

		async hydrateComposerFromMessages(messages: ChatMessage[]) {
			const source = messages.findLast(m => m.role === 'assistant') ?? messages.findLast(m => m.role === 'user');
			if (!source) return;

			const settings = source.request_settings ?? {};
			const modelKey = source.model_key ?? settings.model_key ?? null;
			if (modelKey) {
				const model = await this.findModelByKey(modelKey);
				if (model && model.id !== this.selectedModel?.id) {
					this.setSelectedModel(model);
				}
			}

			this.reasoningEffort = source.reasoning_details?.effort ?? null;
			this.reasoningBudget = source.reasoning_details?.budget_tokens ?? null;
			this.selectedProviderSlug = settings.provider_slug ?? null;
			this.providerRoutingMode = settings.provider_routing_mode === 'lock' ? 'lock' : 'prefer';
			this.enabledTools = Array.isArray(settings.enabled_tools) ? [...settings.enabled_tools] : [];
		},
	},
});
