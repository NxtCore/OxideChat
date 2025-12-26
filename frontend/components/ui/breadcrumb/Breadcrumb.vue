<template>
  <nav :aria-label="ariaLabel" :data-orientation="orientation" :class="cn('flex gap-1.5 break-words text-sm text-muted-foreground', props.class)">
    <slot />
  </nav>
</template>

<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<{
  ariaLabel?: string
  orientation?: 'horizontal' | 'vertical'
  class?: HTMLAttributes['class']
}>(), {
  ariaLabel: 'breadcrumb',
  orientation: 'horizontal',
})
</script>