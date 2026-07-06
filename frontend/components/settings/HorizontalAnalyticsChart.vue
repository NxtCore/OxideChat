<template>
	<div ref="chartRef" class="relative">
		<svg
			:viewBox="`0 0 ${chartWidth} ${chartHeight}`"
			class="w-full overflow-visible"
			:style="{height: `${chartHeight}px`}"
			role="img"
			@mouseleave="hoveredRow = null"
		>
			<g class="text-muted-foreground">
				<line
					v-for="tick in xTicks"
					:key="tick.value"
					:x1="tick.x"
					:x2="tick.x"
					:y1="chartPadding.top"
					:y2="chartHeight - chartPadding.bottom"
					stroke="currentColor"
					stroke-opacity="0.14"
				/>
				<text
					v-for="tick in xTicks"
					:key="`tick-${tick.value}`"
					:x="tick.x"
					:y="chartHeight - 12"
					text-anchor="middle"
					class="fill-current text-[12px] font-medium"
				>
					{{ formatValue(tick.value) }}
				</text>
			</g>

			<g>
				<template v-for="row in chartRows" :key="row.label">
					<text
						:x="chartPadding.left - 12"
						:y="row.y + row.height / 2 + 4"
						text-anchor="end"
						class="fill-muted-foreground text-[12px] font-medium"
					>
						{{ trimLabel(row.label) }}
					</text>
					<rect
						:x="chartPadding.left"
						:y="row.y"
						:width="row.width"
						:height="row.height"
						:fill="color"
						rx="4"
						tabindex="0"
						class="cursor-pointer transition-opacity"
						:class="hoveredRow && hoveredRow.label !== row.label ? 'opacity-55' : 'opacity-100'"
						@mousemove="setHoveredRow($event, row)"
						@focus="setHoveredRow($event, row)"
						@blur="hoveredRow = null"
					/>
				</template>
			</g>
		</svg>

		<div
			v-if="hoveredRow"
			class="pointer-events-none absolute z-20 min-w-56 rounded-lg border border-border bg-popover px-4 py-3 text-popover-foreground shadow-lg"
			:style="{left: `${hoveredRow.x}px`, top: `${hoveredRow.y}px`}"
		>
			<p class="mb-2 truncate text-sm font-semibold text-muted-foreground">{{ hoveredRow.label }}</p>
			<div class="flex items-center justify-between gap-6 text-sm">
				<div class="flex items-center gap-2">
					<span class="h-2 w-2 shrink-0 rounded-full" :style="{backgroundColor: color}" />
					<span class="font-medium">{{ valueLabel }}</span>
				</div>
				<span class="font-bold">{{ formatValue(hoveredRow.value) }}</span>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
type ChartInputRow = {
	label: string;
	value: number;
};

type ChartRow = ChartInputRow & {
	y: number;
	width: number;
	height: number;
};

type HoveredRow = ChartRow & {
	x: number;
	y: number;
};

const props = defineProps<{
	rows: ChartInputRow[];
	valueLabel: string;
	color?: string;
	formatValue: (value: number) => string;
}>();

const chartRef = ref<HTMLElement | null>(null);
const hoveredRow = ref<HoveredRow | null>(null);

const chartWidth = 1000;
const rowHeight = 28;
const rowGap = 12;
const chartPadding = {
	top: 10,
	right: 24,
	bottom: 34,
	left: 170,
};

const color = computed(() => props.color ?? 'var(--primary)');
const chartHeight = computed(() => chartPadding.top + chartPadding.bottom + props.rows.length * rowHeight + Math.max(0, props.rows.length - 1) * rowGap);
const plotWidth = computed(() => chartWidth - chartPadding.left - chartPadding.right);
const maxValue = computed(() => Math.max(1, ...props.rows.map(row => row.value)));

const chartRows = computed<ChartRow[]>(() =>
	props.rows.map((row, index) => ({
		...row,
		y: chartPadding.top + index * (rowHeight + rowGap),
		width: Math.max(2, (row.value / maxValue.value) * plotWidth.value),
		height: rowHeight,
	})),
);

const xTicks = computed(() => {
	const ticks = 3;
	return Array.from({length: ticks + 1}, (_, index) => {
		const value = (maxValue.value / ticks) * index;
		return {
			value,
			x: chartPadding.left + (value / maxValue.value) * plotWidth.value,
		};
	});
});

function setHoveredRow(event: MouseEvent | FocusEvent, row: ChartRow) {
	if (!(event instanceof MouseEvent) || !chartRef.value) {
		hoveredRow.value = {...row, x: 16, y: 16};
		return;
	}
	const bounds = chartRef.value.getBoundingClientRect();
	const tooltipWidth = 250;
	const x = Math.min(event.clientX - bounds.left + 14, Math.max(14, bounds.width - tooltipWidth));
	const y = Math.max(8, event.clientY - bounds.top - 82);
	hoveredRow.value = {...row, x, y};
}

function trimLabel(label: string): string {
	return label.length > 22 ? `${label.slice(0, 20)}...` : label;
}

function formatValue(value: number): string {
	return props.formatValue(value);
}
</script>
