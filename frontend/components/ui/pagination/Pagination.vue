<template>
	<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between rounded-xl p-4 gap-4">
		<div class="flex flex-row gap-2" v-if="showPagination">
			<ShadButton v-if="page > 1" @click="start" variant="secondary" size="icon" class="h-10 px-4">
				<ChevronsLeft class="h-4 w-4" />
			</ShadButton>

			<ShadButton v-if="page !== 1" @click="back" variant="secondary" size="icon" class="h-10 px-4">
				<ChevronLeft class="h-4 w-4" />
			</ShadButton>

			<ShadButton
				v-for="val of pageWindow"
				:key="val"
				@click="selectPage(val)"
				:variant="val === page ? 'default' : 'secondary'"
				size="icon"
				class="h-10 px-4 min-w-10"
				:style="val === page && color ? `background-color: ${color}` : ''"
			>
				{{ val }}
			</ShadButton>

			<ShadButton v-if="hasMore" @click="forward" variant="secondary" size="icon" class="h-10 px-4">
				<ChevronRight class="h-4 w-4" />
			</ShadButton>
		</div>
		<div v-else></div>
	</div>
</template>

<script setup>
import {ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight} from 'lucide-vue-next';
const props = defineProps({
	color: {
		type: String,
		default: null,
	},
	hasMore: {
		type: Boolean,
		default: false,
	},
	hasMore2: {
		type: Boolean,
		default: false,
	},
	data: {
		type: Object,
		required: true,
	},
});

const emit = defineEmits(['update']);

const store = useMainStore();

const page = ref(props.data?.page || 1);
const size = ref(props.data?.size || 25);
const hasMore = ref(props.hasMore);
const hasMore2 = ref(props.hasMore2);

watch(
	() => props.data,
	to => {
		if (to) {
			page.value = to.page;
			size.value = to.size;
		}
	},
	{deep: true}
);

watch(
	() => props.hasMore,
	to => {
		hasMore.value = to;
	}
);

watch(
	() => props.hasMore2,
	to => {
		hasMore2.value = to;
	}
);

const showPagination = computed(() => {
	return page.value > 1 || hasMore.value;
});

const pageWindow = computed(() => {
	const values = [];
	for (let i = page.value - 2; i <= page.value + 2; i++) {
		if (i < 1) continue;
		if (i === page.value) {
			values.push(i);
			continue;
		}
		if (i < page.value) {
			values.push(i);
			continue;
		}
		if (i === page.value + 1 && hasMore.value) values.push(i);
		if (i === page.value + 2 && hasMore2.value) values.push(i);
	}
	return values;
});

const start = () => {
	page.value = 1;
	emit('update', {page: page.value, size: size.value});
};

const selectPage = index => {
	if (index < 1) return;
	page.value = index;
	emit('update', {page: page.value, size: size.value});
};

const back = () => {
	if (page.value === 1) return;
	page.value--;
	emit('update', {page: page.value, size: size.value});
};

const forward = () => {
	if (!hasMore.value) return;
	page.value++;
	emit('update', {page: page.value, size: size.value});
};

const values = () => {
	return {page: page.value, size: size.value ?? 25};
};

const update = new_data => {
	page.value = new_data.page;
	size.value = new_data.size;
};

defineExpose({
	values,
	update,
});
</script>
