<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { alertVariants, type AlertVariants } from '@/components/ui/alert/alert'
import { cn } from '@/lib/utils'

defineOptions({
  inheritAttrs: false,
})

const attrs = useAttrs()

withDefaults(
  defineProps<{
    title: string
    description: string
    variant?: AlertVariants['variant']
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
  <div
    v-bind="delegatedAttrs"
    :class="cn(alertVariants({ variant }), attrs.class as string)"
  >
    <div class="mt-0.5 shrink-0 text-current">
      <slot name="icon" />
    </div>
    <div class="space-y-1">
      <p class="font-semibold text-foreground">{{ title }}</p>
      <p class="leading-6 text-muted-foreground">{{ description }}</p>
    </div>
  </div>
</template>
