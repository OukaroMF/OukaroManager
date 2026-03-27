<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  Boxes,
  Languages,
  LoaderCircle,
  RefreshCcw,
  Save,
  Search,
  Shield,
  ShieldAlert,
  Sparkles,
  TriangleAlert,
} from 'lucide-vue-next'
import { toast } from 'vue-sonner'
import { useI18n } from 'vue-i18n'

import karoLogo from '@/assets/karo.svg'
import { Alert } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { RadioGroup, type RadioOption } from '@/components/ui/radio-group'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { Toaster } from '@/components/ui/toaster'
import {
  getModuleMetadata,
  inspectConfig,
  isKernelSuAvailable,
  replaceConfig,
  requestEdgeToEdge,
  showNativeToast,
} from '@/lib/module-api'
import type { AppMode, InspectPayload, ModuleMetadata } from '@/lib/types'

type AssignmentMap = Record<string, AppMode>

const modePriority: Record<AppMode, number> = {
  priv: 0,
  system: 1,
  none: 2,
}

const { locale, t } = useI18n()

const kernelSupported = ref(isKernelSuAvailable())
const moduleMetadata = ref<ModuleMetadata | null>(null)
const inspectState = ref<InspectPayload | null>(null)
const loading = ref(true)
const refreshing = ref(false)
const saving = ref(false)
const errorMessage = ref<string | null>(null)
const search = ref('')
const originalAssignments = ref<AssignmentMap>({})
const draftAssignments = ref<AssignmentMap>({})
const lastSavedAt = ref<string | null>(null)

function buildAssignments(payload: InspectPayload): AssignmentMap {
  const nextAssignments: AssignmentMap = {}

  payload.systemApp.forEach((packageName) => {
    nextAssignments[packageName] = 'system'
  })
  payload.privApp.forEach((packageName) => {
    nextAssignments[packageName] = 'priv'
  })

  return nextAssignments
}

function errorToMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  return String(error)
}

function formatTime(date: Date) {
  return new Intl.DateTimeFormat(locale.value.startsWith('zh') ? 'zh-CN' : 'en-US', {
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function modeLabel(mode: AppMode) {
  return t(`mode.${mode}`)
}

function modeBadgeVariant(mode: AppMode) {
  if (mode === 'priv') {
    return 'default'
  }

  if (mode === 'system') {
    return 'outline'
  }

  return 'secondary'
}

const modeOptions = computed<RadioOption<AppMode>[]>(() => [
  {
    value: 'none',
    label: t('mode.none'),
    hint: t('mode.noneHint'),
  },
  {
    value: 'system',
    label: t('mode.system'),
    hint: t('mode.systemHint'),
  },
  {
    value: 'priv',
    label: t('mode.priv'),
    hint: t('mode.privHint'),
  },
])

const localeOptions = [
  { value: 'zh-CN', label: '中文' },
  { value: 'en', label: 'EN' },
] as const

const stats = computed(() => {
  const assignedModes = Object.values(draftAssignments.value)

  return {
    installed: inspectState.value?.installedUserApps.length ?? 0,
    configured: assignedModes.filter((mode) => mode !== 'none').length,
    system: assignedModes.filter((mode) => mode === 'system').length,
    priv: assignedModes.filter((mode) => mode === 'priv').length,
    stale: inspectState.value?.missingConfiguredApps.length ?? 0,
  }
})

const dirty = computed(() => {
  const packageNames = new Set([
    ...Object.keys(originalAssignments.value),
    ...Object.keys(draftAssignments.value),
  ])

  for (const packageName of packageNames) {
    if (
      (originalAssignments.value[packageName] ?? 'none') !==
      (draftAssignments.value[packageName] ?? 'none')
    ) {
      return true
    }
  }

  return false
})

const visiblePackages = computed(() => {
  const payload = inspectState.value
  if (!payload) {
    return []
  }

  const query = search.value.trim().toLowerCase()

  return payload.installedUserApps
    .filter((packageName) => !query || packageName.toLowerCase().includes(query))
    .map((packageName) => {
      const currentMode = draftAssignments.value[packageName] ?? 'none'
      const originalMode = originalAssignments.value[packageName] ?? 'none'

      return {
        packageName,
        currentMode,
        originalMode,
        changed: currentMode !== originalMode,
      }
    })
    .sort((left, right) => {
      const priorityDelta =
        modePriority[left.currentMode] - modePriority[right.currentMode]

      if (priorityDelta !== 0) {
        return priorityDelta
      }

      return left.packageName.localeCompare(right.packageName)
    })
})

const missingConfiguredSystem = computed(() => {
  const payload = inspectState.value
  if (!payload) {
    return []
  }

  const missingPackages = new Set(payload.missingConfiguredApps)
  return payload.systemApp.filter((packageName) => missingPackages.has(packageName))
})

const missingConfiguredPriv = computed(() => {
  const payload = inspectState.value
  if (!payload) {
    return []
  }

  const missingPackages = new Set(payload.missingConfiguredApps)
  return payload.privApp.filter((packageName) => missingPackages.has(packageName))
})

const canSave = computed(
  () =>
    kernelSupported.value &&
    dirty.value &&
    !saving.value &&
    !loading.value &&
    !refreshing.value,
)

async function loadState(kind: 'initial' | 'refresh' = 'initial') {
  if (kind === 'initial') {
    loading.value = true
  } else {
    refreshing.value = true
  }

  errorMessage.value = null

  try {
    if (!kernelSupported.value) {
      inspectState.value = null
      moduleMetadata.value = null
      originalAssignments.value = {}
      draftAssignments.value = {}
      return
    }

    requestEdgeToEdge()
    moduleMetadata.value = await getModuleMetadata()

    const payload = await inspectConfig()
    inspectState.value = payload

    const nextAssignments = buildAssignments(payload)
    originalAssignments.value = nextAssignments
    draftAssignments.value = { ...nextAssignments }

    if (kind === 'refresh') {
      const message = t('toasts.refreshed')
      toast.success(message)
      showNativeToast(message)
    }
  } catch (error) {
    errorMessage.value = errorToMessage(error)

    if (kind === 'refresh') {
      const message = t('toasts.loadFailed')
      toast.error(message)
      showNativeToast(message)
    }
  } finally {
    loading.value = false
    refreshing.value = false
  }
}

function updatePackageMode(packageName: string, nextMode: AppMode) {
  draftAssignments.value = {
    ...draftAssignments.value,
    [packageName]: nextMode,
  }
}

function resetDraft() {
  draftAssignments.value = { ...originalAssignments.value }
  errorMessage.value = null
}

function packagesByMode(mode: AppMode) {
  return Object.entries(draftAssignments.value)
    .filter(([, currentMode]) => currentMode === mode)
    .map(([packageName]) => packageName)
    .sort((left, right) => left.localeCompare(right))
}

async function saveChanges() {
  if (!canSave.value) {
    return
  }

  saving.value = true
  errorMessage.value = null

  try {
    const systemPackages = packagesByMode('system')
    const privPackages = packagesByMode('priv')

    await replaceConfig(systemPackages, privPackages)

    originalAssignments.value = { ...draftAssignments.value }
    lastSavedAt.value = formatTime(new Date())

    if (inspectState.value) {
      const installedPackages = new Set(inspectState.value.installedUserApps)
      const configuredPackages = [...systemPackages, ...privPackages]

      inspectState.value = {
        ...inspectState.value,
        systemApp: systemPackages,
        privApp: privPackages,
        missingConfiguredApps: configuredPackages.filter(
          (packageName) => !installedPackages.has(packageName),
        ),
      }
    }

    const message = t('toasts.saved')
    toast.success(message)
    showNativeToast(message)
  } catch (error) {
    errorMessage.value = errorToMessage(error)

    const message = t('toasts.saveFailed')
    toast.error(message)
    showNativeToast(message)
  } finally {
    saving.value = false
  }
}

onMounted(() => {
  void loadState()
})
</script>

<template>
  <div class="relative min-h-screen overflow-hidden">
    <div class="mx-auto flex min-h-screen max-w-7xl flex-col gap-6 px-4 py-5 sm:px-6 lg:px-8">
      <section class="hero-shell overflow-hidden rounded-[32px] border border-border/70 p-6 sm:p-8">
        <div class="relative z-10 flex flex-col gap-8 xl:flex-row xl:items-end xl:justify-between">
          <div class="max-w-3xl">
            <div class="flex items-start gap-4 sm:gap-5">
              <div class="flex h-16 w-16 shrink-0 items-center justify-center rounded-[24px] border border-border/80 bg-card p-2 shadow-sm sm:h-20 sm:w-20 sm:rounded-[28px] sm:p-3">
                <img :src="karoLogo" alt="OukaroManager logo" class="h-full w-full object-contain" />
              </div>

              <div class="min-w-0">
                <Badge class="gap-2 border-primary/20 bg-primary/10 px-3 py-1.5 text-primary" variant="outline">
                  <Sparkles class="h-3.5 w-3.5" />
                  {{ t('header.eyebrow') }}
                </Badge>
                <h1 class="mt-5 max-w-2xl text-4xl font-black tracking-[-0.04em] text-foreground sm:text-5xl">
                  {{ t('title') }}
                </h1>
              </div>
            </div>
            <p class="mt-4 max-w-2xl text-base leading-7 text-foreground/75 sm:text-lg">
              {{ t('subtitle') }}
            </p>
            <div class="mt-5 flex flex-wrap gap-2">
              <Badge :variant="kernelSupported ? 'default' : 'secondary'">
                {{ kernelSupported ? t('header.supported') : t('header.preview') }}
              </Badge>
              <Badge variant="outline">{{ t('header.reboot') }}</Badge>
              <Badge variant="outline">{{ t('header.managedBy') }}</Badge>
            </div>
          </div>

          <Card class="w-full max-w-md bg-background/70">
            <div class="flex items-center justify-between gap-4">
              <div>
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('language.label') }}
                </p>
                <p class="mt-2 text-sm text-foreground/75">
                  {{ kernelSupported ? t('header.supported') : t('header.preview') }}
                </p>
              </div>
              <Languages class="h-5 w-5 text-primary" />
            </div>
            <div class="mt-4 flex flex-wrap gap-2">
              <Button
                v-for="option in localeOptions"
                :key="option.value"
                :variant="locale === option.value ? 'default' : 'outline'"
                size="sm"
                @click="locale = option.value"
              >
                {{ option.label }}
              </Button>
            </div>
          </Card>
        </div>
      </section>

      <div class="grid gap-4">
        <Alert
          v-if="!kernelSupported"
          :description="t('alerts.unsupportedBody')"
          :title="t('alerts.unsupportedTitle')"
          variant="warning"
        >
          <template #icon>
            <ShieldAlert class="h-5 w-5" />
          </template>
        </Alert>

        <Alert
          v-if="errorMessage"
          :description="errorMessage"
          :title="
            saving ? t('alerts.saveFailedTitle') : t('alerts.loadFailedTitle')
          "
          variant="destructive"
        >
          <template #icon>
            <TriangleAlert class="h-5 w-5" />
          </template>
        </Alert>

        <Alert
          v-if="(inspectState?.missingConfiguredApps.length ?? 0) > 0"
          :description="
            t('alerts.missingBody', {
              count: inspectState?.missingConfiguredApps.length ?? 0,
            })
          "
          :title="t('alerts.missingTitle')"
          variant="warning"
        >
          <template #icon>
            <TriangleAlert class="h-5 w-5" />
          </template>
        </Alert>

        <Alert
          v-if="stats.priv > 0"
          :description="t('alerts.privBody')"
          :title="t('alerts.privTitle')"
          variant="warning"
        >
          <template #icon>
            <ShieldAlert class="h-5 w-5" />
          </template>
        </Alert>

        <Alert
          :description="t('alerts.rebootBody')"
          :title="t('alerts.rebootTitle')"
        >
          <template #icon>
            <Shield class="h-5 w-5" />
          </template>
        </Alert>
      </div>

      <div class="grid gap-6 xl:grid-cols-[1.35fr_0.9fr]">
        <Card class="flex min-h-[42rem] flex-col">
          <div class="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div class="max-w-xl">
              <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                {{ t('list.title') }}
              </p>
              <h2 class="mt-2 text-2xl font-bold tracking-tight text-foreground">
                {{ t('list.title') }}
              </h2>
              <p class="mt-2 text-sm leading-6 text-muted-foreground">
                {{ t('list.description') }}
              </p>
            </div>

            <div class="w-full lg:max-w-sm">
              <label class="mb-2 inline-flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                <Search class="h-3.5 w-3.5" />
                {{ t('list.searchLabel') }}
              </label>
              <div class="relative">
                <Search class="pointer-events-none absolute left-4 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  v-model="search"
                  :placeholder="t('list.searchPlaceholder')"
                  class="pl-11"
                />
              </div>
            </div>
          </div>

          <div class="mt-4 flex flex-wrap items-center gap-2">
            <Badge variant="outline">
              {{ t('list.packageCount', { count: stats.installed }) }}
            </Badge>
            <Badge :variant="dirty ? 'default' : 'outline'">
              {{ dirty ? t('summary.dirty') : t('summary.synced') }}
            </Badge>
          </div>

          <Separator class="my-5" />

          <div
            v-if="loading"
            class="flex flex-1 flex-col items-center justify-center gap-3 text-center text-muted-foreground"
          >
            <LoaderCircle class="h-6 w-6 animate-spin text-primary" />
            <p>{{ t('status.loading') }}</p>
          </div>

          <div
            v-else-if="visiblePackages.length === 0"
            class="flex flex-1 items-center justify-center rounded-[28px] border border-dashed border-border/80 bg-background/50 p-8 text-center text-sm leading-6 text-muted-foreground"
          >
            {{ search ? t('list.empty') : t('list.noData') }}
          </div>

          <ScrollArea v-else class="flex-1 pr-1">
            <div class="grid gap-4">
              <Card
                v-for="item in visiblePackages"
                :key="item.packageName"
                class="bg-background/78 p-0"
              >
                <div class="flex flex-col gap-4 p-4">
                  <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                    <div class="min-w-0 space-y-3">
                      <div class="flex flex-wrap items-center gap-2">
                        <Badge :variant="modeBadgeVariant(item.currentMode)">
                          {{ modeLabel(item.currentMode) }}
                        </Badge>
                        <Badge v-if="item.changed" variant="outline">
                          {{ t('labels.changed') }}
                        </Badge>
                      </div>
                      <p class="break-all font-mono text-sm font-semibold text-foreground">
                        {{ item.packageName }}
                      </p>
                    </div>
                  </div>

                  <RadioGroup
                    :model-value="item.currentMode"
                    :options="modeOptions"
                    @update:model-value="(mode) => updatePackageMode(item.packageName, mode)"
                  />
                </div>
              </Card>
            </div>
          </ScrollArea>
        </Card>

        <div class="grid gap-6">
          <Card class="space-y-5">
            <div class="flex items-start justify-between gap-4">
              <div>
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('summary.title') }}
                </p>
                <h2 class="mt-2 text-2xl font-bold tracking-tight text-foreground">
                  {{ t('summary.title') }}
                </h2>
                <p class="mt-2 text-sm leading-6 text-muted-foreground">
                  {{ t('summary.description') }}
                </p>
              </div>
              <Boxes class="mt-1 h-5 w-5 text-primary" />
            </div>

            <div class="grid gap-3 sm:grid-cols-2">
              <div class="rounded-[24px] border border-border/70 bg-background/70 p-4">
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('summary.installed') }}
                </p>
                <p class="mt-3 text-3xl font-black text-foreground">{{ stats.installed }}</p>
              </div>
              <div class="rounded-[24px] border border-border/70 bg-background/70 p-4">
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('summary.configured') }}
                </p>
                <p class="mt-3 text-3xl font-black text-foreground">{{ stats.configured }}</p>
              </div>
              <div class="rounded-[24px] border border-border/70 bg-background/70 p-4">
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('summary.system') }}
                </p>
                <p class="mt-3 text-3xl font-black text-foreground">{{ stats.system }}</p>
              </div>
              <div class="rounded-[24px] border border-border/70 bg-background/70 p-4">
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('summary.priv') }}
                </p>
                <p class="mt-3 text-3xl font-black text-foreground">{{ stats.priv }}</p>
              </div>
            </div>

            <div class="rounded-[24px] border border-primary/15 bg-primary/8 p-4">
              <div class="flex items-center justify-between gap-3">
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('summary.stale') }}
                </p>
                <Badge :variant="stats.stale > 0 ? 'destructive' : 'outline'">
                  {{ stats.stale }}
                </Badge>
              </div>
              <div class="mt-3 flex flex-wrap gap-2" v-if="missingConfiguredSystem.length > 0">
                <Badge v-for="packageName in missingConfiguredSystem" :key="`system-${packageName}`" variant="outline">
                  {{ packageName }}
                </Badge>
              </div>
              <div class="mt-3 flex flex-wrap gap-2" v-if="missingConfiguredPriv.length > 0">
                <Badge v-for="packageName in missingConfiguredPriv" :key="`priv-${packageName}`">
                  {{ packageName }}
                </Badge>
              </div>
            </div>

            <div class="flex flex-col gap-3">
              <Button :disabled="!canSave" size="lg" @click="saveChanges">
                <LoaderCircle v-if="saving" class="h-4 w-4 animate-spin" />
                <Save v-else class="h-4 w-4" />
                {{ saving ? t('actions.saving') : t('actions.save') }}
              </Button>
              <div class="grid gap-3 sm:grid-cols-2">
                <Button
                  :disabled="!dirty || saving"
                  variant="outline"
                  @click="resetDraft"
                >
                  {{ t('actions.reset') }}
                </Button>
                <Button
                  :disabled="refreshing || saving || !kernelSupported"
                  variant="secondary"
                  @click="loadState('refresh')"
                >
                  <LoaderCircle v-if="refreshing" class="h-4 w-4 animate-spin" />
                  <RefreshCcw v-else class="h-4 w-4" />
                  {{ refreshing ? t('status.refreshing') : t('actions.refresh') }}
                </Button>
              </div>
            </div>

            <p v-if="lastSavedAt" class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
              {{ t('summary.savedAt', { time: lastSavedAt }) }}
            </p>
          </Card>

          <Card class="space-y-4 bg-background/72">
            <div class="flex items-start justify-between gap-4">
              <div>
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('header.managedBy') }}
                </p>
                <h2 class="mt-2 text-2xl font-bold tracking-tight text-foreground">
                  {{ moduleMetadata?.name || 'OukaroManager' }}
                </h2>
              </div>
              <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-[18px] border border-border/80 bg-card p-2">
                <img :src="karoLogo" alt="OukaroManager logo" class="h-full w-full object-contain" />
              </div>
            </div>

            <div class="space-y-4">
              <div class="rounded-[24px] border border-border/70 bg-background/70 p-4">
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('header.moduleId') }}
                </p>
                <p class="mt-2 break-all font-mono text-sm text-foreground">
                  {{ moduleMetadata?.id || t('header.modulePending') }}
                </p>
              </div>
              <div class="rounded-[24px] border border-border/70 bg-background/70 p-4">
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('header.version') }}
                </p>
                <p class="mt-2 font-mono text-sm text-foreground">
                  {{ moduleMetadata?.version || t('header.moduleUnavailable') }}
                </p>
              </div>
              <div class="rounded-[24px] border border-border/70 bg-background/70 p-4">
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('header.modulePath') }}
                </p>
                <p class="mt-2 break-all font-mono text-sm text-foreground">
                  {{ moduleMetadata?.moduleDir || t('header.moduleUnavailable') }}
                </p>
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>

    <Toaster />
  </div>
</template>
