<template>
	<div class="rounded-lg border border-border bg-card p-4">
		<p class="text-sm font-medium text-foreground">{{ store.getTranslation('developer.endpoints.title') }}</p>
		<p class="mb-3 text-xs text-muted-foreground">{{ store.getTranslation('developer.endpoints.hint') }}</p>
		<div class="space-y-4">
			<div v-for="endpoint in GATEWAY_ENDPOINTS" :key="endpoint.id" :data-testid="`endpoint-${endpoint.id}`">
				<div class="flex flex-wrap items-center gap-2">
					<span class="w-full text-xs text-muted-foreground sm:w-36">{{ store.getTranslation(endpoint.label) }}</span>
					<code class="min-w-0 flex-1 break-all rounded-md bg-muted px-3 py-1.5 font-mono text-sm text-foreground" :data-testid="`endpoint-url-${endpoint.id}`">{{ origin }}{{ endpoint.path }}</code>
					<ShadButton variant="outline" size="icon" :aria-label="store.getTranslation('developer.reveal.copy')" @click="store.copyToClipboard(`${origin}${endpoint.path}`)">
						<Copy class="h-4 w-4" />
					</ShadButton>
				</div>
				<p v-if="endpoint.hint" class="mt-1 text-xs text-muted-foreground sm:ml-38">{{ store.getTranslation(endpoint.hint) }}</p>
				<p class="mb-2 mt-3 text-xs text-muted-foreground">{{ store.getTranslation('developer.endpoints.routes') }}</p>
				<ul class="grid gap-x-6 gap-y-1 sm:grid-cols-2" :data-testid="`gateway-routes-${endpoint.id}`">
					<li v-for="route in endpoint.routes" :key="route.path" class="flex min-w-0 items-baseline gap-2 text-xs" :data-testid="`gateway-route-${endpoint.id === 'openai' ? '' : `${endpoint.id}-`}${route.path.slice(1).replaceAll('/', '-')}`">
						<code class="shrink-0 font-mono text-foreground"><span class="text-muted-foreground">{{ route.method }}</span> {{ route.path }}</code>
						<span class="truncate text-muted-foreground">{{ store.getTranslation(route.type ? `developer.model_type.${route.type}` : 'developer.endpoints.models') }}</span>
					</li>
				</ul>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Copy} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();
const origin = ref('');

onMounted(() => {
	origin.value = window.location.origin;
});
</script>
