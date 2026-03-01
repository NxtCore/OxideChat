<template>
	<div class="max-w-4xl lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between mb-6">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tabs.models') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.models.description') }}</p>
			</div>
		</div>

		<div class="flex flex-row gap-2 mb-4">
			<ShadInput v-model="search" type="text" :placeholder="store.getTranslation('settings.models.search')" class="flex-1" />
		</div>

		<div v-if="loading" class="flex items-center justify-center py-12 text-muted-foreground">
			<Loader2 class="h-6 w-6 animate-spin" />
		</div>

		<div v-else-if="paginatedModels.length === 0" class="flex items-center justify-center py-12 text-muted-foreground">
			<div class="text-center">
				<Bot class="h-12 w-12 mx-auto mb-4 opacity-50" />
				<p>{{ store.getTranslation('settings.models.no_models') }}</p>
			</div>
		</div>

		<div v-else class="space-y-2">
			<NuxtLink
				v-for="model in paginatedModels"
				:key="model.id"
				:to="`/settings/models/${model.id}`"
				class="block rounded-lg border border-border bg-card px-4 py-3 transition-all hover:border-primary/50 hover:bg-accent/10"
			>
				<div class="flex items-center gap-4">
					<div class="flex h-10 w-10 items-center justify-center rounded-md bg-muted overflow-hidden">
						<img v-if="model.icon" :src="model.icon" class="h-6 w-6 object-cover rounded" />
						<div
							v-else-if="iconStore.getProviderIcon(model.provider.name, model.model_id)?.type === 'svg'"
							v-html="iconStore.getProviderIcon(model.provider.name, model.model_id)!.icon"
							class="h-5 w-5 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full text-muted-foreground"
						/>
						<img
							v-else-if="iconStore.getProviderIcon(model.provider.name, model.model_id)?.type === 'png'"
							:src="iconStore.getProviderIcon(model.provider.name, model.model_id)!.icon"
							alt="Provider icon"
							class="h-5 w-5"
						/>
						<Bot v-else class="h-6 w-6 text-muted-foreground" />
					</div>
					<div class="flex-1 min-w-0">
						<div class="flex items-center gap-2">
							<span class="font-medium text-foreground">{{ model.display_name }}</span>
							<span v-if="model.is_enabled" class="inline-flex items-center rounded-full bg-green-500/10 px-2 py-0.5 text-xs font-medium text-green-600"
								>Active</span
							>
							<span v-else class="inline-flex items-center rounded-full bg-red-500/10 px-2 py-0.5 text-xs font-medium text-red-600">Disabled</span>
						</div>
						<div class="text-sm text-muted-foreground truncate">{{ model.provider.name }} &bull; {{ model.model_id }}</div>
					</div>
					<ChevronRight class="h-5 w-5 text-muted-foreground" />
				</div>
			</NuxtLink>
		</div>

		<AppPagination v-model="page" :has-more="hasMore" />
	</div>
</template>

<script setup lang="ts">
import {Bot, Loader2, ChevronRight} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useIconsStore} from '@/stores/icons';
import {useNuxtApp} from '#app';
import type {ModelListAdmin, PaginatedResponse} from '~/types/chat';

const {$customFetch} = useNuxtApp();

const store = useMainStore();
const iconStore = useIconsStore();

const hasMore = ref(false);
const models = ref<ModelListAdmin[]>([]);
const loading = ref(true);
const search = ref('');
const page = ref(1);
const size = ref(20);
let debounceTimer: ReturnType<typeof setTimeout>;

onMounted(async () => {
	await loadModels();
});

watch(search, newVal => {
	clearTimeout(debounceTimer);
	debounceTimer = setTimeout(() => {
		page.value = 1;
		loadModels();
	}, 750);
});

watch(page, (newVal, oldVal) => {
	loadModels();
});

async function loadModels() {
	loading.value = true;
	try {
		const query = new URLSearchParams({
			page: page.value.toString(),
			size: size.value.toString(),
			query: search.value,
		});
		const res = await $customFetch<PaginatedResponse<ModelListAdmin>>('/api/v1/admin/models?' + query.toString());
		if (res) {
			hasMore.value = res.has_more;
			models.value = res.items;
		}
	} catch (e) {
		console.error(e);
		store.toast(store.getTranslation('settings.models.load_error'), {type: 'error'});
	} finally {
		loading.value = false;
	}
}

const paginatedModels = computed(() => models.value);
</script>
