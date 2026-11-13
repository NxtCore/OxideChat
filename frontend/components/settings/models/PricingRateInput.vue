<template>
	<div class="space-y-1">
		<ShadLabel class="text-xs" :for="testid">{{ store.getTranslation(`pricing.rate.${field}`) }}</ShadLabel>
		<ShadInput :id="testid" v-model="model" type="number" min="0" step="any" :placeholder="store.getTranslation('pricing.inherit')" :data-testid="testid" />
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';

defineProps<{field: string; testid: string}>();

const model = defineModel<string>({required: true});
const store = useMainStore();
</script>
