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
  <section
    v-bind="delegatedAttrs"
    :class="
      cn(
        'rounded-[28px] border border-border/70 bg-card/85 p-5 shadow-[0_28px_90px_-60px_rgba(38,35,28,0.55)] backdrop-blur',
        attrs.class as string,
      )
    "
  >
    <slot />
  </section>
</template>
