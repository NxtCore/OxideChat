import type {ModelList, PaginatedResponse, ProviderTab} from '~/types/chat';

export type ModelFilter = 'all' | 'favorites' | string;

export function useModelPicker(options: {endpoint?: string} = {}) {
	const {$customFetch} = useNuxtApp();
	const endpoint = options.endpoint ?? '/api/v1/models';

	const models = ref<ModelList[]>([]);
	const providers = ref<ProviderTab[]>([]);
	const loading = ref(false);
	const loadingMore = ref(false);
	const searchQuery = ref('');
	const page = ref(1);
	const pageSize = 50;
	const hasMore = ref(false);
	const selectedFilter = ref<ModelFilter>('all');

	let debounceTimer: ReturnType<typeof setTimeout> | null = null;

	const canLoadMore = computed(() => hasMore.value && !loadingMore.value && !loading.value);

	function buildParams(p: number): Record<string, string> {
		const params: Record<string, string> = {
			page: p.toString(),
			size: pageSize.toString(),
		};
		if (searchQuery.value.trim()) {
			params.query = searchQuery.value.trim();
		}
		if (selectedFilter.value === 'favorites') {
			params.is_favorite = 'true';
		} else if (selectedFilter.value !== 'all') {
			params.provider_id = selectedFilter.value;
		}
		return params;
	}

	async function loadProviders() {
		try {
			const res = await $customFetch<ProviderTab[]>('/api/v1/providers');
			providers.value = res ?? [];
		} catch (e) {
			console.error('Failed to fetch model providers:', e);
		}
	}

	async function loadInitial() {
		loading.value = true;
		page.value = 1;
		try {
			const res = await $customFetch<PaginatedResponse<ModelList>>(endpoint, {params: buildParams(1)});
			if (res) {
				models.value = res.items ?? [];
				hasMore.value = res.has_more ?? false;
			}
		} catch (e) {
			console.error('Failed to fetch models:', e);
			models.value = [];
			hasMore.value = false;
		} finally {
			loading.value = false;
		}
	}

	async function loadMore() {
		if (!canLoadMore.value) return;
		loadingMore.value = true;
		const nextPage = page.value + 1;
		try {
			const res = await $customFetch<PaginatedResponse<ModelList>>(endpoint, {params: buildParams(nextPage)});
			if (res) {
				models.value.push(...(res.items ?? []));
				hasMore.value = res.has_more ?? false;
				page.value = nextPage;
			}
		} catch (e) {
			console.error('Failed to load more models:', e);
		} finally {
			loadingMore.value = false;
		}
	}

	function onSearchInput(value: string) {
		searchQuery.value = value;
		if (debounceTimer) clearTimeout(debounceTimer);
		debounceTimer = setTimeout(() => {
			loadInitial();
		}, 300);
	}

	function selectFilter(filter: ModelFilter) {
		if (selectedFilter.value === filter) return;
		selectedFilter.value = filter;
		loadInitial();
	}

	async function init() {
		await Promise.all([loadProviders(), loadInitial()]);
	}

	return {
		models,
		providers,
		loading,
		loadingMore,
		searchQuery,
		hasMore,
		selectedFilter,
		canLoadMore,
		loadProviders,
		loadInitial,
		loadMore,
		onSearchInput,
		selectFilter,
		init,
	};
}
