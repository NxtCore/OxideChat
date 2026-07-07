<template>
	<ShadPopover v-model:open="open">
		<ShadPopoverTrigger as-child>
			<button
				class="flex items-center gap-2 rounded-md border border-border bg-card px-3 py-1.5 text-sm text-foreground transition-colors hover:bg-muted focus:outline-none"
			>
				<CalendarIcon class="h-3.5 w-3.5 text-muted-foreground shrink-0" />
				<span class="font-medium">{{ activeLabel }}</span>
				<span v-if="displayRange" class="text-muted-foreground">{{ displayRange }}</span>
				<ChevronDown class="h-3.5 w-3.5 text-muted-foreground shrink-0 ml-1" />
			</button>
		</ShadPopoverTrigger>
		<ShadPopoverContent class="w-auto p-0" align="end" :side-offset="6">
			<div class="flex">
				<!-- Preset list -->
				<div class="flex flex-col border-r border-border p-2 min-w-36">
					<button
						v-for="preset in presets"
						:key="preset.i18n"
						class="flex items-center rounded-md px-2.5 py-1.5 text-sm transition-colors text-left"
						:class="activeI18nKey === preset.i18n
							? 'bg-primary text-primary-foreground font-medium'
							: 'text-muted-foreground hover:bg-muted hover:text-foreground'"
						@click="applyPreset(preset)"
					>
						{{ store.getTranslation(preset.i18n) }}
					</button>
					<div class="my-1.5 border-t border-border" />
					<button
						class="flex items-center rounded-md px-2.5 py-1.5 text-sm transition-colors text-left"
						:class="activeI18nKey === 'custom'
							? 'bg-muted text-foreground font-medium'
							: 'text-muted-foreground hover:bg-muted hover:text-foreground'"
						@click="showCalendar = !showCalendar"
					>
						{{ store.getTranslation('settings.analytics.custom_range') }}
					</button>
				</div>

				<!-- Range calendar -->
				<div v-if="showCalendar" class="p-3">
					<ShadRangeCalendar
						v-model="calendarValue"
						:number-of-months="1"
						weekday-format="short"
						@update:model-value="onCalendarChange"
					/>
				</div>
			</div>
		</ShadPopoverContent>
	</ShadPopover>
</template>

<script setup lang="ts">
import {CalendarIcon, ChevronDown} from 'lucide-vue-next';
import {CalendarDate} from '@internationalized/date';
import type {DateRange} from 'reka-ui';
import {useMainStore} from '@/stores';

const emit = defineEmits<{
	change: [{from: string; to: string; label: string}];
}>();

const store = useMainStore();
const open = ref(false);
const showCalendar = ref(false);
const activeI18nKey = ref('settings.analytics.preset_last_30d');

const calendarValue = ref<DateRange>({
	start: undefined,
	end: undefined,
});

const presets = [
	{i18n: 'settings.analytics.preset_last_7d', days: 7},
	{i18n: 'settings.analytics.preset_last_30d', days: 30},
	{i18n: 'settings.analytics.preset_last_90d', days: 90},
	{i18n: 'settings.analytics.preset_this_month', days: 0, mode: 'this-month'},
	{i18n: 'settings.analytics.preset_last_month', days: 0, mode: 'last-month'},
	{i18n: 'settings.analytics.preset_this_year', days: 0, mode: 'this-year'},
];

const selectedFrom = ref('');
const selectedTo = ref('');

const activeLabel = computed(() =>
	activeI18nKey.value === 'custom'
		? store.getTranslation('settings.analytics.custom_range')
		: store.getTranslation(activeI18nKey.value),
);

const displayRange = computed(() => {
	if (!selectedFrom.value || !selectedTo.value) return '';
	const fmt = (s: string) => {
		const d = new Date(s + 'T12:00:00');
		return d.toLocaleDateString('en-US', {month: 'short', day: 'numeric'});
	};
	return `${fmt(selectedFrom.value)} – ${fmt(selectedTo.value)}`;
});

function isoDate(d: Date) {
	return d.toISOString().slice(0, 10);
}

function applyPreset(preset: {i18n: string; days: number; mode?: string}) {
	const now = new Date();
	let start: Date;
	let end = new Date(now);

	if (preset.mode === 'this-month') {
		start = new Date(now.getFullYear(), now.getMonth(), 1);
	} else if (preset.mode === 'last-month') {
		start = new Date(now.getFullYear(), now.getMonth() - 1, 1);
		end = new Date(now.getFullYear(), now.getMonth(), 0);
	} else if (preset.mode === 'this-year') {
		start = new Date(now.getFullYear(), 0, 1);
	} else {
		start = new Date();
		start.setDate(start.getDate() - preset.days);
	}

	activeI18nKey.value = preset.i18n;
	selectedFrom.value = isoDate(start);
	selectedTo.value = isoDate(end);
	showCalendar.value = false;

	calendarValue.value = {
		start: new CalendarDate(start.getFullYear(), start.getMonth() + 1, start.getDate()),
		end: new CalendarDate(end.getFullYear(), end.getMonth() + 1, end.getDate()),
	};

	emit('change', {from: selectedFrom.value, to: selectedTo.value, label: store.getTranslation(preset.i18n)});
	open.value = false;
}

function onCalendarChange(range: DateRange | undefined) {
	if (!range?.start || !range?.end) return;

	const start = range.start;
	const end = range.end;
	selectedFrom.value = `${start.year}-${String(start.month).padStart(2, '0')}-${String(start.day).padStart(2, '0')}`;
	selectedTo.value = `${end.year}-${String(end.month).padStart(2, '0')}-${String(end.day).padStart(2, '0')}`;
	activeI18nKey.value = 'custom';

	emit('change', {from: selectedFrom.value, to: selectedTo.value, label: store.getTranslation('settings.analytics.custom_range')});
	open.value = false;
}

onMounted(() => {
	applyPreset(presets[1]);
});
</script>
