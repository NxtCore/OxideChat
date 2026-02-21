<script setup lang="ts">
import type {SelectTriggerProps} from 'reka-ui';
import type {ComputedRef, HTMLAttributes} from 'vue';
import {inject} from 'vue';
import {reactiveOmit} from '@vueuse/core';
import {ChevronDown, X} from 'lucide-vue-next';
import {SelectIcon, SelectTrigger, useForwardProps} from 'reka-ui';
import {cn} from '@/lib/utils';

const props = withDefaults(defineProps<SelectTriggerProps & {class?: HTMLAttributes['class']; size?: 'sm' | 'default'}>(), {size: 'default'});

const delegatedProps = reactiveOmit(props, 'class', 'size');
const forwardedProps = useForwardProps(delegatedProps);

const clearable = inject<ComputedRef<boolean>>('selectClearable');
const hasValue = inject<ComputedRef<boolean>>('selectHasValue');
const clearValue = inject<() => void>('selectClear');
</script>

<template>
	<SelectTrigger
		data-slot="select-trigger"
		:data-size="size"
		v-bind="forwardedProps"
		:class="
			cn(
				'border-input data-[placeholder]:text-muted-foreground [&_svg:not([class*=\'text-\'])]:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:bg-input/30 dark:hover:bg-input/50 flex w-fit items-center justify-between gap-2 rounded-md border bg-transparent px-3 py-2 text-sm whitespace-nowrap shadow-xs transition-[color,box-shadow] outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-9 data-[size=sm]:h-8 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*=\'size-\'])]:size-4',
				props.class
			)
		"
	>
		<slot />
		<div class="flex items-center gap-1">
			<span
				v-if="clearable && hasValue"
				role="button"
				tabindex="-1"
				class="text-muted-foreground hover:text-foreground cursor-pointer rounded-sm transition-opacity"
				@pointerdown.stop.prevent="clearValue"
			>
				<X class="size-4" />
			</span>
			<SelectIcon>
				<ChevronDown class="size-4 opacity-50" />
			</SelectIcon>
		</div>
	</SelectTrigger>
</template>
