<template>
	<div class="rounded-lg border border-border bg-card p-4">
		<p class="text-sm font-medium text-foreground">{{ store.getTranslation('developer.endpoints.title') }}</p>
		<p class="mb-3 text-xs text-muted-foreground">{{ store.getTranslation('developer.endpoints.hint') }}</p>
		<div class="space-y-2">
			<div v-for="endpoint in GATEWAY_ENDPOINTS" :key="endpoint.id" class="flex flex-wrap items-center gap-2" :data-testid="`endpoint-${endpoint.id}`">
				<span class="w-full text-xs text-muted-foreground sm:w-36">{{ store.getTranslation(endpoint.label) }}</span>
				<code class="min-w-0 flex-1 break-all rounded-md bg-muted px-3 py-1.5 font-mono text-sm text-foreground">{{ origin }}{{ endpoint.path }}</code>
				<ShadButton variant="outline" size="icon" :aria-label="store.getTranslation('developer.reveal.copy')" @click="store.copyToClipboard(`${origin}${endpoint.path}`)">
					<Copy class="h-4 w-4" />
				</ShadButton>
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
