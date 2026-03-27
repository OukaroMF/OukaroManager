<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { badgeVariants, type BadgeVariants } from '@/components/ui/badge/badge'
import { cn } from '@/lib/utils'

defineOptions({
  inheritAttrs: false,
})

const attrs = useAttrs()

withDefaults(
  defineProps<{
    variant?: BadgeVariants['variant']
  }>(),
  {
    variant: 'default',
  },
)

const delegatedAttrs = computed(() => {
  const { class: _class, ...rest } = attrs
  return rest
})
</script>

<template>
  <span
    v-bind="delegatedAttrs"
    :class="cn(badgeVariants({ variant }), attrs.class as string)"
  >
    <slot />
  </span>
</template>
