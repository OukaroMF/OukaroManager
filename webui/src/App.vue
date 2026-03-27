<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
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
import { persistLocale } from '@/lib/i18n'
import {
  formatBridgeError,
  getPackageIconUrl,
  getPackagesInfo,
  getRuntimeCapabilities,
  getModuleMetadata,
  inspectConfig,
  isKernelSuAvailable,
  replaceConfig,
  requestEdgeToEdge,
  showNativeToast,
} from '@/lib/module-api'
import type {
  AppMode,
  InspectPayload,
  InspectUserAppsSource,
  ModuleMetadata,
  PackageInfoSummary,
  RuntimeCapabilities,
} from '@/lib/types'

type AssignmentMap = Record<string, AppMode>
interface PersistedDraft {
  moduleKey: string
  baseAssignments: AssignmentMap
  draftAssignments: AssignmentMap
  search: string
}

const DRAFT_STORAGE_PREFIX = 'oukaro.webui.draft'
const DEFAULT_VISIBLE_PACKAGE_COUNT = 10

const modePriority: Record<AppMode, number> = {
  priv: 0,
  system: 1,
  none: 2,
}

const { locale, t } = useI18n()

const runtimeCapabilities = ref<RuntimeCapabilities>(getRuntimeCapabilities())
const kernelSupported = ref(isKernelSuAvailable())
const moduleMetadata = ref<ModuleMetadata | null>(null)
const inspectState = ref<InspectPayload | null>(null)
const loading = ref(true)
const refreshing = ref(false)
const saving = ref(false)
const packageDetailsLoading = ref(false)
const errorMessage = ref<string | null>(null)
const search = ref('')
const packageListExpanded = ref(false)
const originalAssignments = ref<AssignmentMap>({})
const draftAssignments = ref<AssignmentMap>({})
const lastSavedAt = ref<string | null>(null)
const packageDetails = ref<Record<string, PackageInfoSummary>>({})
const brokenIconUrls = ref<Record<string, true>>({})
let packageDetailsRequestId = 0

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
  return formatBridgeError(error)
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

function runtimeLabel(runtime: RuntimeCapabilities['runtime']) {
  if (runtime === 'webuix') {
    return t('header.runtimeWebUiX')
  }

  if (runtime === 'kernelsu') {
    return t('header.runtimeKernelSu')
  }

  return t('header.runtimePreview')
}

function inspectSourceLabel(source: InspectUserAppsSource | undefined) {
  if (!source) {
    return null
  }

  return t(`sources.${source}`)
}

function assignmentsEqual(left: AssignmentMap, right: AssignmentMap) {
  const packageNames = new Set([...Object.keys(left), ...Object.keys(right)])

  for (const packageName of packageNames) {
    if ((left[packageName] ?? 'none') !== (right[packageName] ?? 'none')) {
      return false
    }
  }

  return true
}

function getModuleStorageKey() {
  return moduleMetadata.value?.id || moduleMetadata.value?.moduleDir || runtimeCapabilities.value.runtime
}

function getDraftStorageKey() {
  return `${DRAFT_STORAGE_PREFIX}:${getModuleStorageKey()}`
}

function readPersistedDraft() {
  try {
    const raw = window.localStorage.getItem(getDraftStorageKey())
    if (!raw) {
      return null
    }

    return JSON.parse(raw) as PersistedDraft
  } catch {
    return null
  }
}

function clearPersistedDraft() {
  try {
    window.localStorage.removeItem(getDraftStorageKey())
  } catch {
    // Ignore storage failures in restricted WebView environments.
  }
}

function persistDraftState() {
  if (!moduleMetadata.value) {
    return
  }

  if (!dirty.value) {
    clearPersistedDraft()
    return
  }

  const draft: PersistedDraft = {
    moduleKey: getModuleStorageKey(),
    baseAssignments: { ...originalAssignments.value },
    draftAssignments: { ...draftAssignments.value },
    search: search.value,
  }

  try {
    window.localStorage.setItem(getDraftStorageKey(), JSON.stringify(draft))
  } catch {
    // Ignore storage failures in restricted WebView environments.
  }
}

function restorePersistedDraft() {
  const persisted = readPersistedDraft()
  if (!persisted) {
    return false
  }

  if (persisted.moduleKey !== getModuleStorageKey()) {
    clearPersistedDraft()
    return false
  }

  if (!assignmentsEqual(persisted.baseAssignments, originalAssignments.value)) {
    clearPersistedDraft()
    return false
  }

  draftAssignments.value = { ...persisted.draftAssignments }
  search.value = persisted.search
  return true
}

function resetPackageDetails() {
  packageDetailsRequestId += 1
  packageDetails.value = {}
  packageDetailsLoading.value = false
}

function resetBrokenIcons() {
  brokenIconUrls.value = {}
}

function markIconBroken(iconUrl: string | null | undefined) {
  if (!iconUrl) {
    return
  }

  brokenIconUrls.value = {
    ...brokenIconUrls.value,
    [iconUrl]: true,
  }
}

async function loadPackageDetails(packageNames: string[]) {
  const requestId = ++packageDetailsRequestId

  if (!runtimeCapabilities.value.hasPackageInfo) {
    packageDetails.value = {}
    packageDetailsLoading.value = false
    return
  }

  const missingPackages = [...new Set(packageNames)].filter(
    (packageName) => !packageDetails.value[packageName],
  )

  if (missingPackages.length === 0) {
    packageDetailsLoading.value = false
    return
  }

  packageDetailsLoading.value = true

  try {
    const details = await getPackagesInfo(missingPackages)
    if (requestId !== packageDetailsRequestId) {
      return
    }

    if (!details) {
      return
    }

    packageDetails.value = {
      ...packageDetails.value,
      ...Object.fromEntries(details.map((detail) => [detail.packageName, detail])),
    }
  } catch {
    // Keep already loaded package details when an incremental fetch fails.
  } finally {
    if (requestId === packageDetailsRequestId) {
      packageDetailsLoading.value = false
    }
  }
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
  return !assignmentsEqual(originalAssignments.value, draftAssignments.value)
})

const runtimeName = computed(() => runtimeLabel(runtimeCapabilities.value.runtime))
const runtimeCapabilityLabel = computed(() =>
  kernelSupported.value ? t('header.capabilityFull') : t('header.capabilityLimited'),
)
const hasLimitedBridge = computed(
  () => runtimeCapabilities.value.hasBridge && !kernelSupported.value,
)
const inspectWarnings = computed(() => inspectState.value?.warnings ?? [])
const inspectSourceName = computed(() =>
  inspectSourceLabel(inspectState.value?.installedUserAppsSource),
)
const hasInspectFallback = computed(() => {
  const source = inspectState.value?.installedUserAppsSource
  return source === 'packagesXmlAndRestrictions' || source === 'packagesXmlBestEffort'
})
const inspectFallbackDetails = computed(() =>
  inspectWarnings.value.join(' ') || t('alerts.inspectFallbackDefault'),
)

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
      const detail = packageDetails.value[packageName]
      const appLabel = detail?.appLabel?.trim() || null
      const versionLabel =
        detail?.versionName?.trim() ||
        (typeof detail?.versionCode === 'number' ? `vc ${detail.versionCode}` : null)
      const iconUrl = detail?.iconUrl || getPackageIconUrl(packageName)

      return {
        packageName,
        appLabel,
        versionLabel,
        iconUrl: iconUrl && !brokenIconUrls.value[iconUrl] ? iconUrl : null,
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

const displayedPackages = computed(() =>
  packageListExpanded.value
    ? visiblePackages.value
    : visiblePackages.value.slice(0, DEFAULT_VISIBLE_PACKAGE_COUNT),
)

const displayedPackageNamesKey = computed(() =>
  displayedPackages.value.map((item) => item.packageName).join('\u0000'),
)

const hasHiddenPackages = computed(
  () => visiblePackages.value.length > DEFAULT_VISIBLE_PACKAGE_COUNT,
)

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
  runtimeCapabilities.value = getRuntimeCapabilities()
  kernelSupported.value = isKernelSuAvailable()

  if (kind === 'initial') {
    loading.value = true
  } else {
    refreshing.value = true
  }

  errorMessage.value = null

  try {
    if (!kernelSupported.value) {
      resetPackageDetails()
      resetBrokenIcons()
      inspectState.value = null
      moduleMetadata.value = null
      packageListExpanded.value = false
      originalAssignments.value = {}
      draftAssignments.value = {}
      return
    }

    requestEdgeToEdge()
    moduleMetadata.value = await getModuleMetadata()

    const payload = await inspectConfig()
    inspectState.value = payload
    packageListExpanded.value = false
    resetPackageDetails()
    resetBrokenIcons()

    const nextAssignments = buildAssignments(payload)
    originalAssignments.value = nextAssignments
    draftAssignments.value = { ...nextAssignments }

    if (restorePersistedDraft()) {
      const message = t('toasts.draftRestored')
      toast.success(message)
      showNativeToast(message)
    }

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
  clearPersistedDraft()
}

function togglePackageListExpanded() {
  packageListExpanded.value = !packageListExpanded.value
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
    clearPersistedDraft()
  } catch (error) {
    errorMessage.value = errorToMessage(error)

    const message = t('toasts.saveFailed')
    toast.error(message)
    showNativeToast(message)
  } finally {
    saving.value = false
  }
}

watch(locale, (nextLocale) => {
  persistLocale(String(nextLocale))
})

watch([draftAssignments, search], () => {
  persistDraftState()
}, { deep: true })

watch(search, () => {
  packageListExpanded.value = false
})

watch(displayedPackageNamesKey, (joinedPackageNames) => {
  if (!joinedPackageNames || !kernelSupported.value) {
    return
  }

  void loadPackageDetails(displayedPackages.value.map((item) => item.packageName))
})

onMounted(() => {
  requestEdgeToEdge()
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
                {{ runtimeName }}
              </Badge>
              <Badge variant="outline">{{ runtimeCapabilityLabel }}</Badge>
              <Badge variant="outline">{{ t('header.reboot') }}</Badge>
              <Badge variant="outline">{{ t('header.managedBy') }}</Badge>
            </div>
          </div>

          <Card class="w-full max-w-md bg-background/70">
            <div class="flex items-center justify-between gap-4">
              <div>
                <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {{ t('header.runtime') }}
                </p>
                <p class="mt-2 text-sm text-foreground/75">
                  {{ runtimeName }}
                </p>
              </div>
              <Languages class="h-5 w-5 text-primary" />
            </div>
            <div class="mt-4 flex flex-wrap gap-2">
              <p class="w-full text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                {{ t('language.label') }}
              </p>
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
          v-else-if="hasLimitedBridge"
          :description="t('alerts.limitedBridgeBody')"
          :title="t('alerts.limitedBridgeTitle')"
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
          v-if="hasInspectFallback"
          :description="
            t('alerts.inspectFallbackBody', {
              source: inspectSourceName,
              details: inspectFallbackDetails,
            })
          "
          :title="t('alerts.inspectFallbackTitle')"
          variant="warning"
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
            <Badge v-if="inspectSourceName" variant="outline">
              {{ t('list.sourceLabel', { source: inspectSourceName }) }}
            </Badge>
            <Badge :variant="dirty ? 'default' : 'outline'">
              {{ dirty ? t('summary.dirty') : t('summary.synced') }}
            </Badge>
            <Badge v-if="hasHiddenPackages" variant="outline">
              {{
                t('list.limitedCount', {
                  shown: displayedPackages.length,
                  total: visiblePackages.length,
                })
              }}
            </Badge>
            <Badge
              v-if="runtimeCapabilities.hasPackageInfo"
              :variant="packageDetailsLoading ? 'outline' : 'secondary'"
            >
              {{
                packageDetailsLoading
                  ? t('list.detailsLoading')
                  : t('list.detailsReady')
              }}
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
                v-for="item in displayedPackages"
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
                        <Badge v-if="item.versionLabel" variant="secondary">
                          {{ item.versionLabel }}
                        </Badge>
                      </div>
                      <div class="flex items-start gap-3">
                        <div class="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-[18px] border border-border/70 bg-card">
                          <img
                            v-if="item.iconUrl"
                            :src="item.iconUrl"
                            :alt="item.appLabel || item.packageName"
                            class="h-9 w-9 object-contain"
                            loading="lazy"
                            @error="markIconBroken(item.iconUrl)"
                          />
                          <span
                            v-else
                            class="text-lg font-black uppercase tracking-[0.08em] text-muted-foreground"
                          >
                            {{ (item.appLabel || item.packageName).slice(0, 1) }}
                          </span>
                        </div>

                        <div class="min-w-0">
                          <p
                            class="truncate text-sm font-semibold text-foreground"
                            :title="item.appLabel || item.packageName"
                          >
                            {{ item.appLabel || item.packageName }}
                          </p>
                          <p class="mt-1 break-all font-mono text-xs text-muted-foreground">
                            {{ item.packageName }}
                          </p>
                        </div>
                      </div>
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

            <div v-if="hasHiddenPackages" class="mt-4 flex justify-center">
              <Button variant="outline" @click="togglePackageListExpanded">
                {{ packageListExpanded ? t('actions.showLess') : t('actions.showMore') }}
              </Button>
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
                  {{ t('header.runtime') }}
                </p>
                <p class="mt-2 font-mono text-sm text-foreground">
                  {{ runtimeName }}
                </p>
              </div>
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
