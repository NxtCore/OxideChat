<template>
  <BreadcrumbItem>
    <span 
      role="link" 
      aria-disabled="true" 
      aria-current="page" 
      :class="cn('font-normal text-foreground', props.class)"
    >
      <slot />
    </span>
  </BreadcrumbItem>
</template>

<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { cn } from '@/lib/utils'
import BreadcrumbItem from './BreadcrumbItem.vue'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()
</script>