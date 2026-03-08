<template>
	<div class="rounded-lg border border-border" :class="enabled ? 'border-border' : 'border-dashed border-border/50 opacity-60'">
		<div class="flex items-center justify-between px-3 py-2.5">
			<div class="flex items-center gap-1.5 min-w-0">
				<ShadLabel class="text-sm font-medium cursor-pointer select-none" @click="$emit('toggle')">
					{{ label }}
				</ShadLabel>
				<ShadTooltipProvider v-if="tooltip">
					<ShadTooltip>
						<ShadTooltipTrigger as-child>
							<Info class="h-3.5 w-3.5 text-muted-foreground cursor-help flex-shrink-0" />
						</ShadTooltipTrigger>
						<ShadTooltipContent side="top" class="max-w-xs text-xs">
							{{ tooltip }}
						</ShadTooltipContent>
					</ShadTooltip>
				</ShadTooltipProvider>
			</div>
			<ShadButton variant="ghost" size="icon" class="h-7 w-7 flex-shrink-0" @click="$emit('toggle')">
				<Minus v-if="enabled" class="h-3.5 w-3.5 text-destructive" />
				<Plus v-else class="h-3.5 w-3.5 text-muted-foreground" />
			</ShadButton>
		</div>
		<div v-if="enabled" class="px-3 pb-3">
			<slot />
		</div>
	</div>
</template>

<script setup lang="ts">
import {Info, Plus, Minus} from 'lucide-vue-next';

defineProps<{
	label: string;
	tooltip?: string;
	enabled: boolean;
}>();

defineEmits<{
	toggle: [];
}>();
</script>
