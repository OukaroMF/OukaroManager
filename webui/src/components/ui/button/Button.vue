<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { buttonVariants, type ButtonVariants } from '@/components/ui/button/button'
import { cn } from '@/lib/utils'

defineOptions({
  inheritAttrs: false,
})

const attrs = useAttrs()

withDefaults(
  defineProps<{
    variant?: ButtonVariants['variant']
    size?: ButtonVariants['size']
    type?: 'button' | 'submit' | 'reset'
  }>(),
  {
    variant: 'default',
    size: 'default',
    type: 'button',
  },
)

const delegatedAttrs = computed(() => {
  const { class: _class, ...rest } = attrs
  return rest
})
</script>

<template>
  <button
    v-bind="delegatedAttrs"
    :class="cn(buttonVariants({ variant, size }), attrs.class as string)"
    :type="type"
  >
    <slot />
  </button>
</template>
