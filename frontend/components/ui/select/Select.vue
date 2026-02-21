<script setup lang="ts">
import type {SelectRootEmits, SelectRootProps} from 'reka-ui';
import {SelectRoot, useForwardPropsEmits} from 'reka-ui';
import {computed, provide, ref, watch} from 'vue';
import {reactiveOmit} from '@vueuse/core';

const props = withDefaults(defineProps<SelectRootProps & {clearable?: boolean}>(), {clearable: false});
const emits = defineEmits<SelectRootEmits>();

type ModelValue = SelectRootProps['modelValue'];
const delegatedProps = reactiveOmit(props, 'clearable');
const forwarded = useForwardPropsEmits(delegatedProps, emits);
const internalValue = ref<ModelValue | undefined>((props.modelValue ?? props.defaultValue) as ModelValue | undefined);
watch(
	() => props.modelValue,
	val => {
		internalValue.value = val as ModelValue | undefined;
	}
);
function onValueChange(val: ModelValue) {
	internalValue.value = val;
}
const selectHasValue = computed(() => {
	const value = internalValue.value;
	if (Array.isArray(value)) {
		return value.length > 0;
	}
	return Boolean(value);
});
provide(
	'selectClearable',
	computed(() => props.clearable)
);
provide('selectHasValue', selectHasValue);
provide('selectClear', () => {
	internalValue.value = undefined;
	emits('update:modelValue', undefined as any);
});
</script>

<template>
	<SelectRoot data-slot="select" v-bind="forwarded" @update:model-value="onValueChange">
		<slot />
	</SelectRoot>
</template>
