<template>
  <li :class="cn('inline-flex items-center gap-1.5', props.class)">
    <slot />
  </li>
</template>

<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()
</script>