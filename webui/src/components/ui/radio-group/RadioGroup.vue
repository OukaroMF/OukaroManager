<script setup lang="ts" generic="T extends string">
import type { RadioOption } from '@/components/ui/radio-group/types'

defineProps<{
  modelValue: T
  options: RadioOption<T>[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: T]
}>()
</script>

<template>
  <div class="grid gap-2 sm:grid-cols-3">
    <label
      v-for="option in options"
      :key="option.value"
      :class="
        modelValue === option.value
          ? 'border-primary/60 bg-primary/10 text-foreground shadow-[0_10px_30px_-22px_hsl(var(--primary)/0.7)]'
          : 'border-border/70 bg-background/70 text-muted-foreground hover:border-primary/30 hover:bg-accent/40'
      "
      class="relative flex min-w-0 cursor-pointer flex-col rounded-2xl border px-3 py-3 transition-all duration-200"
    >
      <input
        :checked="modelValue === option.value"
        class="sr-only"
        type="radio"
        @change="emit('update:modelValue', option.value)"
      />
      <span class="text-sm font-semibold">{{ option.label }}</span>
      <span v-if="option.hint" class="mt-1 text-xs leading-5 text-muted-foreground">
        {{ option.hint }}
      </span>
    </label>
  </div>
</template>
