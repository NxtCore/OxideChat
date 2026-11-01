<template>
	<div class="rounded-lg border transition-colors" :class="enabled ? 'border-border bg-card' : 'border-dashed border-border/50 bg-muted/20'">
		<div class="flex items-center justify-between px-3 pt-3" :class="hasBody ? 'pb-2' : 'pb-3'">
			<div class="flex items-center gap-1.5">
				<span class="text-sm font-medium" :class="enabled ? 'text-foreground' : 'text-muted-foreground'">
					{{ label }}
				</span>
				<ShadTooltipProvider>
					<ShadTooltip>
						<ShadTooltipTrigger as-child>
							<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help" />
						</ShadTooltipTrigger>
						<ShadTooltipContent side="top" class="max-w-xs text-xs">
							{{ tooltip }}
						</ShadTooltipContent>
					</ShadTooltip>
				</ShadTooltipProvider>
			</div>
			<div class="flex items-center gap-2">
				<span v-if="enabled && display" class="text-sm font-mono font-medium text-primary tabular-nums">{{ display }}</span>
				<slot name="extra" />
				<ShadButton variant="ghost" size="icon" class="h-6 w-6" @click="$emit('toggle')">
					<Minus v-if="enabled" class="h-3.5 w-3.5 text-muted-foreground hover:text-destructive" />
					<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
				</ShadButton>
			</div>
		</div>
		<div v-if="hasBody" class="px-3 pb-3 space-y-2">
			<template v-if="min !== undefined">
				<ShadSlider :model-value="[modelValue ?? min]" :min="min" :max="max" :step="step" @update:model-value="$emit('update:modelValue', $event?.[0] ?? null)" />
				<div class="flex justify-between text-[10px] text-muted-foreground">
					<span v-for="l in labels" :key="l">{{ l }}</span>
				</div>
			</template>
			<slot />
		</div>
	</div>
</template>

<script setup lang="ts">
import {Info, Minus, Plus} from 'lucide-vue-next';

const props = defineProps<{
	label: string;
	tooltip: string;
	enabled: boolean;
	display?: string;
	modelValue?: number | null;
	min?: number;
	max?: number;
	step?: number;
	labels?: string[];
}>();

defineEmits<{
	toggle: [];
	'update:modelValue': [value: number | null];
}>();

const slots = useSlots();
const hasBody = computed(() => props.enabled && (props.min !== undefined || !!slots.default));
</script>
