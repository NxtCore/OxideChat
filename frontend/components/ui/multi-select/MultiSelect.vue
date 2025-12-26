<template>
	<div>
		<ShadSelect v-model="internal_value" multiple>
			<ShadSelectTrigger class="w-full bg-neutral-950 border-neutral-800 focus:ring-yellow-500/50">
				<ShadSelectValue :placeholder="placeholder">
					<span class="truncate">
						{{ selected_items.length > 0 ? display_text : placeholder }}
					</span>
				</ShadSelectValue>
			</ShadSelectTrigger>
			<ShadSelectContent class="max-h-60">
				<div class="p-2 sticky -top-2 z-10 bg-popover border-b">
					<ShadInput
						v-model="search_query"
						placeholder="Search..."
						@click.stop
						@input="search_query = $event.target.value"
						class="bg-background border-border" 
					/>
				</div>
				<ShadSelectGroup>
					<ShadSelectItem v-for="item in filtered_items" :key="item.value" :value="item.value" @click.prevent="toggle_item(item)">
						<div class="flex items-center gap-2 flex-1">
							<component v-if="item.customContent" :is="item.customContent" />
							<div class="flex flex-1 items-center gap-2">
								<img v-if="item.image" :src="item.image" :alt="item.label" class="w-6 h-6 rounded-full object-cover" />
								<div v-if="item.fallback && !item.image" class="w-6 h-6 rounded-full bg-neutral-600 flex items-center justify-center text-xs">
									{{ item.fallback }}
								</div>
								<div class="flex flex-col">
									<span>{{ item.label.split(' (')[0] }}</span>
									<span v-if="item.discriminator" class="text-xs text-neutral-400">
										{{ item.label.split(' (')[1]?.slice(0, -1) }}#{{ item.discriminator }}
									</span>
									<span v-else-if="item.label.includes(' (')" class="text-xs text-neutral-400">
										{{ item.label.split(' (')[1]?.slice(0, -1) }}
									</span>
								</div>
							</div>
						</div>
					</ShadSelectItem>
				</ShadSelectGroup>
			</ShadSelectContent>
		</ShadSelect>
	
	</div>
</template>

<script setup>
import {ref, computed, watch} from 'vue';
import {Check} from 'lucide-vue-next';

const props = defineProps({
	modelValue: {
		type: Array,
		default: () => [],
	},
	items: {
		type: Array,
		default: () => [],
	},
	placeholder: {
		type: String,
		default: 'Select items',
	},
	displayText: {
		type: String,
		default: null,
	},
});

const emit = defineEmits(['update:modelValue']);

const search_query = ref('');
const internal_value = ref([]);

const selected_items = computed(() => {
	return props.items.filter(item => props.modelValue.includes(item.value));
});

const filtered_items = computed(() => {
	if (!search_query.value) return props.items;
	return props.items.filter(item => item.label.toLowerCase().includes(search_query.value.toLowerCase()));
});

const display_text = computed(() => {
	if (props.displayText) return props.displayText;
	if (selected_items.value.length === 0) return '';
	if (selected_items.value.length === 1) return selected_items.value[0].label;
	return `${selected_items.value.length} selected`;
});

const is_selected = value => {
	return props.modelValue.includes(value);
};

const toggle_item = item => {
	const new_value = [...props.modelValue];
	const index = new_value.indexOf(item.value);

	if (index > -1) {
		new_value.splice(index, 1);
	} else {
		new_value.push(item.value);
	}

	emit('update:modelValue', new_value);
};

// Watch for changes in props.modelValue to keep internal_value in sync
watch(
	() => props.modelValue,
	newValue => {
		internal_value.value = newValue;
	},
	{immediate: true}
);
</script>
