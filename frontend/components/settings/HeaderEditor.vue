<template>
	<div class="space-y-2">
		<div class="flex items-center justify-between">
			<Label class="text-xs text-muted-foreground">{{ store.getTranslation('mcp.headers') }}</Label>
			<ShadButton variant="ghost" size="sm" type="button" class="h-7 gap-1 text-xs" @click="addHeader">
				<Plus class="h-3 w-3" />
				{{ store.getTranslation('mcp.headers_add') }}
			</ShadButton>
		</div>

		<div v-if="headers.length === 0" class="rounded-md border border-dashed border-border px-3 py-2 text-xs text-muted-foreground">
			{{ store.getTranslation('mcp.headers_empty') }}
		</div>

		<div v-else class="space-y-2">
			<div v-for="(header, idx) in headers" :key="idx" class="flex items-center gap-2">
				<Input
					:model-value="header.key"
					:placeholder="store.getTranslation('mcp.headers_name')"
					class="flex-1 font-mono text-xs"
					@update:model-value="(val: string) => updateKey(idx, val)"
				/>
				<Input
					:model-value="header.value"
					:placeholder="store.getTranslation('mcp.headers_value')"
					class="flex-1 font-mono text-xs"
					@update:model-value="(val: string) => updateValue(idx, val)"
				/>
				<ShadButton
					variant="ghost"
					size="icon"
					type="button"
					class="h-8 w-8 shrink-0 text-destructive hover:text-destructive"
					:title="store.getTranslation('mcp.headers_remove')"
					@click="removeHeader(idx)"
				>
					<Trash2 class="h-3.5 w-3.5" />
				</ShadButton>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {ref, watch} from 'vue';
import {Plus, Trash2} from 'lucide-vue-next';
import {Input} from '@/components/ui/input';
import {Label} from '@/components/ui/label';
import {useMainStore} from '@/stores';

const props = defineProps<{modelValue: string}>();
const emit = defineEmits<{(e: 'update:modelValue', value: string): void}>();

const store = useMainStore();

interface HeaderPair {
	key: string;
	value: string;
}

const headers = ref<HeaderPair[]>([]);

function parseModel(value: string): HeaderPair[] {
	const trimmed = value?.trim();
	if (!trimmed) return [];
	try {
		const parsed = JSON.parse(trimmed);
		if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
			return Object.entries(parsed).map(([key, val]) => ({key, value: String(val ?? '')}));
		}
	} catch {
		return [];
	}
	return [];
}

function serialize(pairs: HeaderPair[]): string {
	const obj: Record<string, string> = {};
	for (const pair of pairs) {
		const key = pair.key.trim();
		if (key) obj[key] = pair.value;
	}
	return JSON.stringify(obj);
}

function emitChange() {
	emit('update:modelValue', serialize(headers.value));
}

function addHeader() {
	headers.value.push({key: '', value: ''});
}

function removeHeader(idx: number) {
	headers.value.splice(idx, 1);
	emitChange();
}

function updateKey(idx: number, val: string) {
	headers.value[idx].key = val;
	emitChange();
}

function updateValue(idx: number, val: string) {
	headers.value[idx].value = val;
	emitChange();
}

watch(
	() => props.modelValue,
	newValue => {
		const incoming = serialize(headers.value);
		if (incoming === newValue) return;
		headers.value = parseModel(newValue);
	},
	{immediate: true}
);
</script>
