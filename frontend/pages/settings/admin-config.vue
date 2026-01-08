<template>
	<div class="max-w-4xl">
		<div class="rounded-lg border border-border bg-card p-6">
			<div class="mb-6">
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tabs.admin_config') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.admin_config.description') }}</p>
			</div>

			<div class="flex items-center justify-center py-12 text-muted-foreground">
				<div class="text-center">
					<Cog class="h-12 w-12 mx-auto mb-4 opacity-50" />
					<p>{{ store.getTranslation('settings.admin_config.coming_soon') }}</p>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Cog} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();
</script>
