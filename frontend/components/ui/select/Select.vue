<script setup lang="ts">
import type { SelectRootEmits, SelectRootProps } from "reka-ui"
import { SelectRoot, useForwardPropsEmits } from "reka-ui"
import { computed, provide, ref, watch } from "vue"
import { reactiveOmit } from "@vueuse/core"

const props = withDefaults(
  defineProps<SelectRootProps & { clearable?: boolean }>(),
  { clearable: false },
)
const emits = defineEmits<SelectRootEmits>()

const delegatedProps = reactiveOmit(props, "clearable")
const forwarded = useForwardPropsEmits(delegatedProps, emits)

const internalValue = ref<string | undefined>(props.modelValue ?? props.defaultValue)

watch(() => props.modelValue, (val) => {
  internalValue.value = val
})

function onValueChange(val: string) {
  internalValue.value = val
}

provide("selectClearable", computed(() => props.clearable))
provide("selectHasValue", computed(() => Boolean(internalValue.value)))
provide("selectClear", () => {
  internalValue.value = undefined
  emits("update:modelValue", "")
})
</script>

<template>
  <SelectRoot
    data-slot="select"
    v-bind="forwarded"
    @update:model-value="onValueChange"
  >
    <slot />
  </SelectRoot>
</template>
