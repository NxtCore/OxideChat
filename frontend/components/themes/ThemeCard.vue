<script setup lang="ts">
import type { FetchedTheme } from '~/types/chat';
import { extractThemeColors } from '~/lib/theme-utils';
import { Trash2, AlertCircle } from 'lucide-vue-next';

const props = defineProps<{
	theme: FetchedTheme;
	isSelected: boolean;
	mode: 'light' | 'dark';
	canDelete?: boolean;
}>();

const emit = defineEmits<{
	(e: 'select'): void;
	(e: 'delete'): void;
}>();

const colors = computed(() => {
	if (props.theme.error) return [];
	return extractThemeColors(props.theme.preset.cssVars, props.mode);
});
</script>

<template>
	<div
		class="relative group cursor-pointer rounded-lg border-2 p-3 transition-all hover:shadow-md"
		:class="{
			'border-primary bg-primary/5': isSelected,
			'border-border hover:border-primary/50': !isSelected,
			'opacity-60': theme.error,
		}"
		@click="emit('select')"
	>
		<div class="flex items-center justify-between mb-2">
			<span class="text-sm font-medium truncate">{{ theme.name }}</span>
			<div class="flex items-center gap-1">
				<AlertCircle v-if="theme.error" class="w-4 h-4 text-destructive" />
				<button
					v-if="canDelete"
					class="opacity-0 group-hover:opacity-100 p-1 hover:bg-destructive/10 rounded transition-opacity"
					@click.stop="emit('delete')"
				>
					<Trash2 class="w-3.5 h-3.5 text-destructive" />
				</button>
			</div>
		</div>

		<div v-if="theme.error" class="text-xs text-destructive truncate">
			{{ theme.error }}
		</div>

		<div v-else class="flex gap-1">
			<div
				v-for="(color, index) in colors"
				:key="index"
				class="w-6 h-6 rounded-md border border-border/50"
				:style="{ backgroundColor: color }"
			/>
		</div>
	</div>
</template>
