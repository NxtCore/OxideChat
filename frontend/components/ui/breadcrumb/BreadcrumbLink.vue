<template>
	<BreadcrumbItem>
		<NuxtLink v-if="to" :to="to" :class="cn('transition-colors hover:text-foreground', props.class)">
			<slot />
		</NuxtLink>
		<span v-else :class="cn('text-foreground', props.class)">
			<slot />
		</span>
	</BreadcrumbItem>
</template>

<script setup lang="ts">
import type {HTMLAttributes} from 'vue';
import {cn} from '@/lib/utils';
import BreadcrumbItem from './BreadcrumbItem.vue';

const props = defineProps<{
	to?: any;
	class?: HTMLAttributes['class'];
}>();
</script>
