<template>
	<ShadPopover v-model:open="open">
		<ShadPopoverTrigger as-child>
			<ShadButton variant="outline" role="combobox" :aria-expanded="open" :disabled="disabled" class="w-full justify-between font-normal">
				<span class="truncate" :class="modelValue ? 'text-foreground' : 'text-muted-foreground'">{{ modelValue || store.getTranslation('developer.playground.pick_model') }}</span>
				<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
			</ShadButton>
		</ShadPopoverTrigger>
		<ShadPopoverContent class="w-(--reka-popover-trigger-width) p-0" align="start">
			<div class="border-b border-border p-2">
				<ShadInput
					v-model="query"
					autocomplete="off"
					:placeholder="store.getTranslation('developer.model_search.placeholder')"
					:aria-label="store.getTranslation('developer.model_search.placeholder')"
					@keydown.down.prevent="move(1)"
					@keydown.up.prevent="move(-1)"
					@keydown.enter.prevent="pickActive"
				/>
			</div>
			<ul ref="list" role="listbox" class="max-h-60 overflow-y-auto p-1">
				<li
					v-for="(entry, index) in filtered"
					:key="entry"
					role="option"
					:aria-selected="entry === modelValue"
					:data-index="index"
					class="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm text-foreground"
					:class="index === activeIndex ? 'bg-accent text-accent-foreground' : ''"
					@mouseenter="activeIndex = index"
					@click="pick(entry)"
				>
					<Check class="h-4 w-4 shrink-0" :class="entry === modelValue ? 'opacity-100' : 'opacity-0'" />
					<span class="break-all">{{ entry }}</span>
				</li>
				<li v-if="!filtered.length" class="px-2 py-6 text-center text-sm text-muted-foreground">{{ store.getTranslation('developer.model_search.empty') }}</li>
			</ul>
		</ShadPopoverContent>
	</ShadPopover>
</template>

<script setup lang="ts">
import {Check, ChevronsUpDown} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const props = defineProps<{models: string[]; modelValue: string; disabled?: boolean}>();
const emit = defineEmits<{'update:modelValue': [value: string]}>();
const store = useMainStore();

const open = ref(false);
const query = ref('');
const activeIndex = ref(0);
const list = ref<HTMLElement | null>(null);

const filtered = computed(() => {
	const needle = query.value.trim().toLowerCase();
	return needle ? props.models.filter(entry => entry.toLowerCase().includes(needle)) : props.models;
});

function scrollToActive() {
	nextTick(() => list.value?.querySelector(`[data-index="${activeIndex.value}"]`)?.scrollIntoView({block: 'nearest'}));
}

function move(step: number) {
	if (!filtered.value.length) return;
	activeIndex.value = (activeIndex.value + step + filtered.value.length) % filtered.value.length;
	scrollToActive();
}

function pick(entry: string) {
	emit('update:modelValue', entry);
	open.value = false;
}

function pickActive() {
	const entry = filtered.value[activeIndex.value];
	if (entry) pick(entry);
}

watch(query, () => {
	activeIndex.value = 0;
});

watch(open, (isOpen) => {
	if (!isOpen) return;
	query.value = '';
	activeIndex.value = Math.max(0, props.models.indexOf(props.modelValue));
	scrollToActive();
});
</script>
