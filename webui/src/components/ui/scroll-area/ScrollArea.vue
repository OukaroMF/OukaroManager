<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { cn } from '@/lib/utils'

defineOptions({
  inheritAttrs: false,
})

const attrs = useAttrs()

const delegatedAttrs = computed(() => {
  const { class: _class, ...rest } = attrs
  return rest
})
</script>

<template>
  <div v-bind="delegatedAttrs" :class="cn('overflow-y-auto', attrs.class as string)">
    <slot />
  </div>
</template>
