<template>
	<ShadBadge :variant="variant" :class="extra">{{ store.getTranslation(`developer.status.${status}`) }}</ShadBadge>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';
import type {DeveloperKey} from '~/types/developer';

const props = defineProps<{apiKey: DeveloperKey}>();
const store = useMainStore();
const status = computed(() => keyStatus(props.apiKey));
const variant = computed(() => (status.value === 'active' ? 'default' : status.value === 'revoked' ? 'destructive' : 'secondary'));
const extra = computed(() => (status.value === 'expired' ? 'text-destructive' : ''));
</script>
