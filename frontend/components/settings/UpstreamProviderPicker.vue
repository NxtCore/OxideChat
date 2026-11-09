<template>
	<div class="space-y-3">
		<div class="flex items-center gap-3">
			<ShadSwitch :model-value="restricted" :disabled="disabled" :data-testid="`${testId}-restrict`" @update:model-value="setRestricted" />
			<ShadLabel class="text-sm">{{ store.getTranslation('settings.provider_allowlist.restrict') }}</ShadLabel>
		</div>
		<template v-if="restricted">
			<div class="rounded-md border border-border">
				<div class="flex items-center gap-2 border-b border-border p-2">
					<ShadInput
						v-model="query"
						class="h-8"
						:disabled="disabled"
						:placeholder="store.getTranslation('settings.provider_allowlist.placeholder')"
						:data-testid="`${testId}-input`"
						@keydown.enter.prevent="addTyped"
					/>
					<span class="shrink-0 text-xs text-muted-foreground" :data-testid="`${testId}-count`">
						{{ store.getTranslation('settings.provider_allowlist.selected', {count: modelValue?.length ?? 0}) }}
					</span>
				</div>
				<div class="max-h-72 overflow-y-auto p-1" :data-testid="`${testId}-list`">
					<label
						v-for="option in visibleOptions"
						:key="option.slug"
						class="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
						:data-testid="`${testId}-option-${option.slug}`"
					>
						<ShadCheckbox :model-value="isSelected(option.slug)" :disabled="disabled" @update:model-value="toggle(option.slug, $event === true)" />
						<span class="min-w-0 flex-1 truncate">
							{{ option.name || option.slug }}
							<span v-if="option.name && option.name.toLowerCase() !== option.slug" class="ml-1 text-xs text-muted-foreground">{{ option.slug }}</span>
						</span>
						<span v-if="location(option)" class="shrink-0 text-xs text-muted-foreground">{{ location(option) }}</span>
					</label>
					<button
						v-if="customSlug"
						type="button"
						class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
						:disabled="disabled"
						:data-testid="`${testId}-add`"
						@click="addTyped"
					>
						<Plus class="h-4 w-4" />
						{{ store.getTranslation('settings.provider_allowlist.add_custom', {slug: customSlug}) }}
					</button>
					<p v-if="!visibleOptions.length && !customSlug" class="px-2 py-1.5 text-xs text-muted-foreground">
						{{ store.getTranslation('settings.provider_allowlist.none_known') }}
					</p>
				</div>
			</div>
			<p v-if="!modelValue?.length" class="text-xs text-destructive" :data-testid="`${testId}-empty`">
				{{ store.getTranslation('settings.provider_allowlist.empty') }}
			</p>
		</template>
	</div>
</template>

<script setup lang="ts">
import {Plus} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import type {UpstreamProviderResponse} from '~/types/generated/UpstreamProviderResponse';

const props = defineProps<{modelValue: string[] | null; providers: UpstreamProviderResponse[]; within?: string[] | null; disabled?: boolean; testId: string}>();
const emit = defineEmits<{'update:modelValue': [value: string[] | null]}>();

const store = useMainStore();
const query = ref('');

const restricted = computed(() => props.modelValue !== null);

const options = computed(() => {
	const offered = props.within ? props.providers.filter(provider => slugWithin(props.within ?? [], provider.slug)) : props.providers;
	const listed = new Set(offered.map(provider => provider.slug));
	const extra = [...new Set([...(props.modelValue ?? []), ...(props.within ?? [])])].filter(slug => !listed.has(slug));
	const all: UpstreamProviderResponse[] = [...offered, ...extra.map(slug => ({slug, name: null, headquarters: null, datacenters: []}))];
	return all.sort((a, b) => (a.name || a.slug).localeCompare(b.name || b.slug));
});

const visibleOptions = computed(() => {
	const term = query.value.trim().toLowerCase();
	if (!term) return options.value;
	return options.value.filter(option => option.slug.includes(term) || option.name?.toLowerCase().includes(term));
});

const customSlug = computed(() => {
	const slug = query.value.trim().toLowerCase();
	if (!/^[a-z0-9\-_./]{1,64}$/.test(slug) || (props.within && !slugWithin(props.within, slug))) return null;
	return options.value.some(option => option.slug === slug) ? null : slug;
});

function location(option: UpstreamProviderResponse) {
	if (!option.headquarters && !option.datacenters.length) return '';
	return store.getTranslation('settings.provider_allowlist.location', {
		headquarters: option.headquarters ?? '?',
		datacenters: option.datacenters.join(', ') || '?',
	});
}

function isSelected(slug: string) {
	return props.modelValue?.includes(slug) ?? false;
}

function setRestricted(value: boolean) {
	emit('update:modelValue', value ? [] : null);
}

function toggle(slug: string, selected: boolean) {
	const current = (props.modelValue ?? []).filter(value => value !== slug);
	emit('update:modelValue', selected ? [...current, slug].sort() : current);
}

function addTyped() {
	const typed = query.value.trim().toLowerCase();
	const slug = customSlug.value ?? (visibleOptions.value.length === 1 ? visibleOptions.value[0].slug : null) ?? (options.value.some(option => option.slug === typed) ? typed : null);
	if (!slug) return;
	toggle(slug, true);
	query.value = '';
}
</script>
