<script setup lang="ts">
withDefaults(
  defineProps<{
    orientation?: 'horizontal' | 'vertical'
  }>(),
  {
    orientation: 'horizontal',
  },
)
</script>

<template>
  <div
    :aria-orientation="orientation"
    :class="
      orientation === 'horizontal'
        ? 'h-px w-full bg-border/80'
        : 'h-full w-px bg-border/80'
    "
    role="separator"
  />
</template>
