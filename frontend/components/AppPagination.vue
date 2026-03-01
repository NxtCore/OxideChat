<template>
	<div v-if="currentPage > 1 || hasMore" class="flex flex-col w-full mt-6">
		<ShadPagination :class="cn('justify-start', props.class)">
			<ShadPaginationContent>
				<ShadPaginationPrevious @click="onPrevious" :disabled="currentPage === 1" />
				<span class="flex items-center gap-1 px-2 text-sm text-foreground">{{ currentPage }}</span>
				<ShadPaginationNext @click="onNext" :disabled="!hasMore" />
			</ShadPaginationContent>
		</ShadPagination>
	</div>
</template>

<script setup lang="ts">
import type {HTMLAttributes} from 'vue';
import {cn} from '@/lib/utils';

const props = withDefaults(
	defineProps<{
		hasMore: boolean;
		modelValue?: number;
		class?: HTMLAttributes['class'];
	}>(),
	{
		modelValue: 1,
	}
);

const emit = defineEmits<{
	(e: 'update:modelValue', page: number): void;
	(e: 'pageChange', page: number): void;
}>();

const currentPage = computed(() => props.modelValue);

function onPageClick(page: number) {
	emit('update:modelValue', page);
	emit('pageChange', page);
}

function onPrevious() {
	if (currentPage.value > 1) {
		onPageClick(currentPage.value - 1);
	}
}

function onNext() {
	if (props.hasMore) {
		onPageClick(currentPage.value + 1);
	}
}
</script>
