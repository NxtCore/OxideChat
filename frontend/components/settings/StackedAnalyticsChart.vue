<template>
	<div ref="chartRef" class="relative">
		<svg
			:viewBox="`0 0 ${chartWidth} ${chartHeight}`"
			class="h-[280px] w-full overflow-visible"
			role="img"
			@mouseleave="hoveredBar = null"
		>
			<g class="text-muted-foreground">
				<line
					v-for="tick in yTicks"
					:key="tick.value"
					:x1="chartPadding.left"
					:x2="chartWidth - chartPadding.right"
					:y1="tick.y"
					:y2="tick.y"
					stroke="currentColor"
					stroke-opacity="0.14"
				/>
				<text
					v-for="tick in yTicks"
					:key="`label-${tick.value}`"
					:x="chartPadding.left - 14"
					:y="tick.y + 4"
					text-anchor="end"
					class="fill-current text-[12px] font-medium"
				>
					{{ formatTick(tick.value) }}
				</text>
			</g>

			<g>
				<template v-for="bar in chartBars" :key="bar.day">
					<rect
						v-for="segment in bar.segments"
						:key="`${bar.day}-${segment.key}`"
						:x="bar.x"
						:y="segment.y"
						:width="bar.width"
						:height="segment.height"
						:fill="segment.color"
						class="transition-opacity"
						:class="hoveredBar && hoveredBar.day !== bar.day ? 'opacity-35' : 'opacity-100'"
						rx="3"
					/>
					<rect
						:x="bar.x"
						:y="chartPadding.top"
						:width="bar.width"
						:height="plotHeight"
						fill="transparent"
						class="cursor-pointer"
						@mousemove="setHoveredBar($event, bar)"
					/>
					<text
						:x="bar.x + bar.width / 2"
						:y="chartHeight - 12"
						text-anchor="middle"
						class="fill-muted-foreground text-[12px] font-semibold"
					>
						{{ bar.day }}
					</text>
				</template>
			</g>
		</svg>

		<div
			v-if="hoveredBar"
			class="pointer-events-none absolute z-20 min-w-52 rounded-lg border border-border bg-popover px-4 py-3 text-popover-foreground shadow-lg"
			:style="{left: `${hoveredBar.x}px`, top: `${hoveredBar.y}px`}"
		>
			<p class="mb-2.5 text-sm font-semibold text-muted-foreground">{{ hoveredBar.day }}</p>
			<div class="space-y-1.5">
				<div
					v-for="segment in hoveredBarSortedSegments"
					:key="segment.key"
					class="flex items-center justify-between gap-6 text-sm"
				>
					<div class="flex min-w-0 items-center gap-2">
						<span class="h-2 w-2 shrink-0 rounded-full" :style="{backgroundColor: segment.color}" />
						<span class="truncate text-muted-foreground">{{ segment.name }}</span>
					</div>
					<span class="font-medium tabular-nums">{{ formatValue(segment.value) }}</span>
				</div>
			</div>
			<div class="mt-2.5 flex items-center justify-between border-t border-border pt-2.5 text-sm">
				<span class="font-medium text-foreground">Total</span>
				<span class="font-bold tabular-nums">{{ formatValue(hoveredBar.total) }}</span>
			</div>
		</div>

		<div class="mt-3 flex flex-wrap items-center justify-center gap-x-5 gap-y-2">
			<button
				v-for="item in legendItems"
				:key="item.key"
				type="button"
				class="flex min-w-0 items-center gap-2 text-xs transition-opacity hover:opacity-100"
				:class="item.enabled ? 'opacity-100' : 'opacity-35'"
				@click="toggleKey(item.key)"
			>
				<span class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{backgroundColor: item.color}" />
				<span class="truncate text-muted-foreground">{{ item.name }}</span>
			</button>
		</div>
	</div>
</template>

<script setup lang="ts">
type ChartRow = Record<string, number | string>;

type ChartCategory = {
	name: string;
	color?: string | string[];
};

type ChartSegment = {
	day: string;
	key: string;
	name: string;
	value: number;
	color: string;
	y: number;
	height: number;
};

type HoveredBar = {
	day: string;
	segments: ChartSegment[];
	total: number;
	x: number;
	y: number;
};

const props = defineProps<{
	data: ChartRow[];
	categories: Record<string, ChartCategory>;
	formatValue: (value: number) => string;
}>();

const chartRef = ref<HTMLElement | null>(null);
const disabledKeys = ref<string[]>([]);
const hoveredBar = ref<HoveredBar | null>(null);

const chartWidth = 1000;
const chartHeight = 280;
const chartPadding = {
	top: 12,
	right: 16,
	bottom: 42,
	left: 42,
};

const categoryKeys = computed(() => Object.keys(props.categories));

const enabledKeys = computed(() => {
	const disabled = new Set(disabledKeys.value);
	const keys = categoryKeys.value.filter(key => !disabled.has(key));
	return keys.length ? keys : categoryKeys.value;
});

const maxTotal = computed(() => {
	const max = props.data.reduce((currentMax, row) => {
		const total = enabledKeys.value.reduce((sum, key) => sum + Number(row[key] ?? 0), 0);
		return Math.max(currentMax, total);
	}, 0);
	return Math.max(1, max);
});

const plotHeight = computed(() => chartHeight - chartPadding.top - chartPadding.bottom);
const plotWidth = computed(() => chartWidth - chartPadding.left - chartPadding.right);

const chartBars = computed(() => {
	const count = Math.max(1, props.data.length);
	const slotWidth = plotWidth.value / count;
	const gap = Math.min(14, slotWidth * 0.18);
	const width = Math.max(2, slotWidth - gap);

	return props.data.map((row, index) => {
		let stackY = chartHeight - chartPadding.bottom;
		const day = String(row.day ?? row.label ?? '');
		const segments = enabledKeys.value
			.map(key => {
				const value = Number(row[key] ?? 0);
				const height = value > 0 ? (value / maxTotal.value) * plotHeight.value : 0;
				stackY -= height;
				return {
					day,
					key,
					name: props.categories[key]?.name ?? key,
					value,
					color: categoryColor(props.categories[key]),
					y: stackY,
					height,
				};
			})
			.filter(segment => segment.height > 0);

		return {
			day,
			x: chartPadding.left + index * slotWidth + gap / 2,
			width,
			segments,
		};
	});
});

const hoveredBarSortedSegments = computed(() => {
	if (!hoveredBar.value) return [];
	return [...hoveredBar.value.segments].sort((a, b) => b.value - a.value);
});

const yTicks = computed(() => {
	const ticks = 3;
	return Array.from({length: ticks + 1}, (_, index) => {
		const value = (maxTotal.value / ticks) * index;
		const y = chartHeight - chartPadding.bottom - (value / maxTotal.value) * plotHeight.value;
		return {value, y};
	}).reverse();
});

const legendItems = computed(() => {
	const disabled = new Set(disabledKeys.value);
	return categoryKeys.value.map(key => ({
		key,
		name: props.categories[key]?.name ?? key,
		color: categoryColor(props.categories[key]),
		enabled: !disabled.has(key),
	}));
});

watch(categoryKeys, keys => {
	disabledKeys.value = disabledKeys.value.filter(key => keys.includes(key));
});

function categoryColor(category?: ChartCategory): string {
	const color = category?.color;
	if (Array.isArray(color)) return color[0] ?? '#6b7280';
	return color ?? '#6b7280';
}

function setHoveredBar(event: MouseEvent, bar: {day: string; segments: ChartSegment[]; x: number; width: number}) {
	if (!chartRef.value) return;
	const bounds = chartRef.value.getBoundingClientRect();
	const tooltipWidth = 230;
	const x = Math.min(event.clientX - bounds.left + 14, Math.max(14, bounds.width - tooltipWidth));
	const y = Math.max(8, event.clientY - bounds.top - 100);
	const total = bar.segments.reduce((sum, s) => sum + s.value, 0);
	hoveredBar.value = {
		day: bar.day,
		segments: bar.segments,
		total,
		x,
		y,
	};
}

function toggleKey(key: string) {
	const disabled = new Set(disabledKeys.value);
	if (disabled.has(key)) {
		disabled.delete(key);
	} else {
		disabled.add(key);
	}
	if (disabled.size >= categoryKeys.value.length) {
		disabled.delete(key);
	}
	disabledKeys.value = Array.from(disabled);
	hoveredBar.value = null;
}

function formatTick(value: number): string {
	if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
	if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
	if (value >= 10) return String(Math.round(value));
	if (value >= 1) return Number.isInteger(value) ? String(value) : value.toFixed(1);
	return value.toFixed(2);
}
</script>
