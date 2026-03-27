<script setup lang="ts">
import { Toaster as SonnerToaster } from 'vue-sonner'
</script>

<template>
  <SonnerToaster
    :duration="3200"
    close-button
    position="top-center"
    rich-colors
    theme="light"
  />
</template>
