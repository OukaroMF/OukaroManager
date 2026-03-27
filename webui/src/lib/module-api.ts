import type {
  InspectPayload,
  ModuleMetadata,
  PackageInfoSummary,
  RuntimeCapabilities,
} from '@/lib/types'

declare global {
  interface Window {
    ksu?: KernelSuBridge
    kernelsu?: KernelSuBridge
    [key: string]: unknown
  }
}

interface KernelSuBridge {
  exec?: (command: string, options?: string | null, callbackFunc?: string) => unknown
  moduleInfo?: () => unknown
  toast?: (message: string) => unknown
  enableEdgeToEdge?: (enable: boolean) => unknown
  mmrl?: () => unknown
  listPackages?: (type: string) => unknown
  getPackagesInfo?: (packages: string | string[]) => unknown
  exit?: () => unknown
}

interface WxuGlobalBridge {
  require?: (module: string) => unknown
}

interface WxuModuleBridge {
  getId?: () => unknown
  getModuleDir?: () => unknown
}

interface WxuPackageManagerBridge {
  getApplicationIcon?: (packageName: string, flags?: number, userId?: number) => unknown
  getApplicationInfo?: (packageName: string, flags?: number, userId?: number) => unknown
}

interface ExecOptions {
  cwd?: string
  env?: Record<string, string>
}

interface ExecResult {
  errno: number
  stdout: string
  stderr: string
}

const INSPECT_TIMEOUT_MS = 15_000
const REPLACE_TIMEOUT_MS = 20_000
const URL_FETCH_TIMEOUT_MS = 5_000
const SYSTEM_USER_ID = 0
const APPLICATION_INFO_FLAG_SYSTEM = 1 << 0

let moduleMetadataPromise: Promise<ModuleMetadata> | null = null

function getGlobalScope() {
  return window as Window & { global?: WxuGlobalBridge }
}

function ensureKernelSuAlias() {
  if (typeof window === 'undefined') {
    return
  }

  if (typeof window.ksu === 'undefined' && typeof window.kernelsu !== 'undefined') {
    window.ksu = window.kernelsu
  }

  if (typeof window.kernelsu === 'undefined' && typeof window.ksu !== 'undefined') {
    window.kernelsu = window.ksu
  }
}

function getBridge(): KernelSuBridge | null {
  if (typeof window === 'undefined') {
    return null
  }

  ensureKernelSuAlias()

  if (window.ksu && typeof window.ksu === 'object') {
    return window.ksu
  }

  if (window.kernelsu && typeof window.kernelsu === 'object') {
    return window.kernelsu
  }

  return null
}

function getWxuGlobal(): WxuGlobalBridge | null {
  if (typeof window === 'undefined') {
    return null
  }

  const globalBridge = getGlobalScope().global
  if (globalBridge && typeof globalBridge.require === 'function') {
    return globalBridge
  }

  return null
}

function getWxuModule(): WxuModuleBridge | null {
  const globalBridge = getWxuGlobal()
  if (!globalBridge?.require) {
    return null
  }

  try {
    const moduleBridge = globalBridge.require('wx:module')
    if (moduleBridge && typeof moduleBridge === 'object') {
      return moduleBridge as WxuModuleBridge
    }
  } catch {
    // Ignore optional WebUIX utility plugin failures.
  }

  return null
}

function getWxuPackageManager(): WxuPackageManagerBridge | null {
  const globalBridge = getWxuGlobal()
  if (!globalBridge?.require) {
    return null
  }

  try {
    const packageManager = globalBridge.require('wx:pm')
    if (packageManager && typeof packageManager === 'object') {
      return packageManager as WxuPackageManagerBridge
    }
  } catch {
    // Ignore optional WebUIX utility plugin failures.
  }

  return null
}

function detectRuntime(bridge: KernelSuBridge | null) {
  if (!bridge) {
    return 'preview' as const
  }

  try {
    if (typeof bridge.mmrl === 'function' && bridge.mmrl() === true) {
      return 'webuix' as const
    }
  } catch {
    // Ignore runtime probe failures and fall back to generic KernelSU mode.
  }

  return 'kernelsu' as const
}

function shellQuote(value: string) {
  return `'${value.replace(/'/g, `'\"'\"'`)}'`
}

function errorToMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message
  }

  return String(error)
}

function commandError(stderr: string, stdout: string, errno: number) {
  const message = stderr.trim() || stdout.trim()
  return new Error(message || `Command failed with exit code ${errno}`)
}

function parseModuleMetadata(raw: unknown): ModuleMetadata {
  const parsed =
    typeof raw === 'string'
      ? (JSON.parse(raw) as ModuleMetadata)
      : (raw as ModuleMetadata | null)

  if (!parsed?.moduleDir) {
    throw new Error('KernelSU did not provide a moduleDir value.')
  }

  return parsed
}

function parsePackageInfoList(raw: unknown) {
  if (typeof raw === 'string') {
    return JSON.parse(raw) as PackageInfoSummary[]
  }

  if (Array.isArray(raw)) {
    return raw as PackageInfoSummary[]
  }

  throw new Error('KernelSU did not provide a valid package info payload.')
}

function parseJsonObject(raw: unknown) {
  if (typeof raw === 'string') {
    try {
      return parseJsonObject(JSON.parse(raw))
    } catch {
      return null
    }
  }

  if (raw && typeof raw === 'object') {
    return raw as Record<string, unknown>
  }

  return null
}

function pickString(...values: unknown[]) {
  for (const value of values) {
    if (typeof value === 'string' && value.trim().length > 0) {
      return value.trim()
    }
  }

  return undefined
}

function pickNumber(...values: unknown[]) {
  for (const value of values) {
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value
    }

    if (typeof value === 'string' && value.trim().length > 0) {
      const parsed = Number(value)
      if (Number.isFinite(parsed)) {
        return parsed
      }
    }
  }

  return undefined
}

function guessBase64ImageMime(base64: string) {
  if (base64.startsWith('iVBOR')) {
    return 'image/png'
  }

  if (base64.startsWith('/9j/')) {
    return 'image/jpeg'
  }

  if (base64.startsWith('UklGR')) {
    return 'image/webp'
  }

  return 'image/png'
}

function base64ToDataUrl(raw: unknown) {
  if (typeof raw !== 'string') {
    return null
  }

  const normalized = raw.trim()
  if (!normalized) {
    return null
  }

  if (normalized.startsWith('data:')) {
    return normalized
  }

  return `data:${guessBase64ImageMime(normalized)};base64,${normalized}`
}

function normalizePackageInfoSummary(
  raw: unknown,
  fallbackPackageName: string,
  iconUrl?: string | null,
): PackageInfoSummary | null {
  const parsed = parseJsonObject(raw)
  if (!parsed) {
    return null
  }

  const packageName = pickString(parsed.packageName) ?? fallbackPackageName
  const appLabel = pickString(
    parsed.appLabel,
    parsed.label,
    parsed.nonLocalizedLabel,
    parsed.name,
  )
  const versionName = pickString(parsed.versionName)
  const versionCode = pickNumber(parsed.versionCode, parsed.longVersionCode)
  const uid = pickNumber(parsed.uid)
  const isSystem =
    typeof parsed.isSystem === 'boolean'
      ? parsed.isSystem
      : (() => {
          const flags = pickNumber(parsed.flags)
          if (typeof flags === 'number') {
            return (flags & APPLICATION_INFO_FLAG_SYSTEM) !== 0
          }

          return null
        })()

  const detail: PackageInfoSummary = {
    packageName,
    appLabel,
    versionName,
    versionCode,
    iconUrl: iconUrl ?? null,
    isSystem,
    uid: typeof uid === 'number' ? uid : null,
  }

  return detail
}

function isPromiseLike<T>(value: unknown): value is PromiseLike<T> {
  return (
    typeof value === 'object' &&
    value !== null &&
    'then' in value &&
    typeof (value as PromiseLike<T>).then === 'function'
  )
}

function normalizeExecResult(
  errno: unknown,
  stdout: unknown,
  stderr: unknown,
): ExecResult {
  const normalizedErrno =
    typeof errno === 'number' && Number.isFinite(errno) ? errno : Number.parseInt(String(errno), 10)

  return {
    errno: Number.isFinite(normalizedErrno) ? normalizedErrno : -1,
    stdout: typeof stdout === 'string' ? stdout : String(stdout ?? ''),
    stderr: typeof stderr === 'string' ? stderr : String(stderr ?? ''),
  }
}

function parseDirectExecResult(raw: unknown): ExecResult | null {
  if (typeof raw === 'string') {
    try {
      return parseDirectExecResult(JSON.parse(raw))
    } catch {
      return null
    }
  }

  if (!raw || typeof raw !== 'object') {
    return null
  }

  const candidate = raw as Partial<ExecResult>
  if (!('errno' in candidate) && !('stdout' in candidate) && !('stderr' in candidate)) {
    return null
  }

  return normalizeExecResult(candidate.errno, candidate.stdout, candidate.stderr)
}

function resolveRuntimeAssetUrl(relativePath: string) {
  return new URL(relativePath, window.location.href).toString()
}

function getWebUiXPackageIconUrl(packageName: string) {
  return resolveRuntimeAssetUrl(`.package/${encodeURIComponent(packageName)}/icon.png`)
}

async function fetchJsonWithTimeout(url: string, timeoutMs: number) {
  if (typeof fetch !== 'function') {
    return null
  }

  const controller = new AbortController()
  const timer = window.setTimeout(() => controller.abort(), timeoutMs)

  try {
    const response = await fetch(url, { signal: controller.signal })
    if (!response.ok) {
      return null
    }

    return await response.json()
  } catch {
    return null
  } finally {
    window.clearTimeout(timer)
  }
}

async function execBridge(
  command: string,
  options: ExecOptions = {},
  timeoutMs = INSPECT_TIMEOUT_MS,
) {
  const bridge = getBridge()
  if (!bridge?.exec) {
    throw new Error('KernelSU exec API is unavailable in this environment.')
  }

  const callbackName = `__oukaro_exec_${Date.now()}_${Math.random().toString(16).slice(2)}`

  return new Promise<ExecResult>((resolve, reject) => {
    let settled = false

    const finalizeResolve = (result: ExecResult) => {
      if (settled) {
        return
      }

      settled = true
      window.clearTimeout(timer)
      cleanup()
      resolve(result)
    }

    const finalizeReject = (error: unknown) => {
      if (settled) {
        return
      }

      settled = true
      window.clearTimeout(timer)
      cleanup()
      reject(error)
    }

    const cleanup = () => {
      delete window[callbackName]
    }

    const timer = window.setTimeout(() => {
      finalizeReject(new Error(`KernelSU exec for \`${command}\` timed out after ${timeoutMs}ms`))
    }, timeoutMs)

    window[callbackName] = (errno: unknown, stdout: unknown, stderr: unknown) => {
      finalizeResolve(normalizeExecResult(errno, stdout, stderr))
    }

    try {
      const result = bridge.exec?.(command, JSON.stringify(options), callbackName)
      if (isPromiseLike<unknown>(result)) {
        void Promise.resolve(result).then(
          (value) => {
            const directResult = parseDirectExecResult(value)
            if (directResult) {
              finalizeResolve(directResult)
            }
          },
          (error) => {
            finalizeReject(error)
          },
        )
        return
      }

      const directResult = parseDirectExecResult(result)
      if (directResult) {
        finalizeResolve(directResult)
      }
    } catch (error) {
      finalizeReject(error)
    }
  })
}

async function runOkrmng(argumentsText: string, timeoutMs: number) {
  const metadata = await getModuleMetadata()
  const moduleDir = metadata.moduleDir.replace(/\/+$/, "")
  const okrmngPath = `${moduleDir}/okrmng`

  const result = await execBridge(`${shellQuote(okrmngPath)} ${argumentsText}`, {
    cwd: moduleDir,
  }, timeoutMs)

  if (result.errno !== 0) {
    throw commandError(result.stderr, result.stdout, result.errno)
  }

  return result.stdout.trim()
}

export function getRuntimeCapabilities(): RuntimeCapabilities {
  const bridge = getBridge()
  const runtime = detectRuntime(bridge)
  const wxuModule = getWxuModule()
  const wxuPackageManager = getWxuPackageManager()

  return {
    runtime,
    hasBridge: bridge !== null,
    hasExec: typeof bridge?.exec === 'function',
    hasModuleInfo: typeof bridge?.moduleInfo === 'function',
    hasWxuModule: wxuModule !== null,
    hasWxuPackageManager: wxuPackageManager !== null,
    hasToast: typeof bridge?.toast === 'function',
    hasEdgeToEdge: typeof bridge?.enableEdgeToEdge === 'function',
    hasListPackages: typeof bridge?.listPackages === 'function' || runtime === 'webuix',
    hasPackageInfo:
      typeof bridge?.getPackagesInfo === 'function' || wxuPackageManager !== null || runtime === 'webuix',
    hasExit: typeof bridge?.exit === 'function',
  }
}

export function isKernelSuAvailable() {
  const capabilities = getRuntimeCapabilities()
  return capabilities.hasExec && (capabilities.hasModuleInfo || capabilities.hasWxuModule)
}

export function requestEdgeToEdge() {
  const bridge = getBridge()

  if (!bridge?.enableEdgeToEdge) {
    return
  }

  try {
    bridge.enableEdgeToEdge(true)
  } catch {
    // Ignore optional runtime helpers when previewing in non-KernelSU environments.
  }
}

export async function getModuleMetadata() {
  const capabilities = getRuntimeCapabilities()
  if (!capabilities.hasModuleInfo && !capabilities.hasWxuModule) {
    throw new Error('KernelSU/WebUIX module metadata APIs are unavailable in this environment.')
  }

  if (!moduleMetadataPromise) {
    moduleMetadataPromise = Promise.resolve()
      .then(async () => {
        const bridgeMetadata = getBridge()?.moduleInfo?.()
        if (typeof bridgeMetadata !== 'undefined') {
          return parseModuleMetadata(await Promise.resolve(bridgeMetadata))
        }

        const wxuModule = getWxuModule()
        const moduleDir = pickString(wxuModule?.getModuleDir?.())
        if (!moduleDir) {
          throw new Error('WebUIX did not provide a moduleDir value.')
        }

        return {
          id: pickString(wxuModule?.getId?.()),
          moduleDir,
        } satisfies ModuleMetadata
      })
      .catch((error) => {
        moduleMetadataPromise = null
        throw error
      })
  }

  return moduleMetadataPromise
}

export async function inspectConfig() {
  const stdout = await runOkrmng('inspect --json', INSPECT_TIMEOUT_MS)
  return JSON.parse(stdout) as InspectPayload
}

export async function replaceConfig(systemPackages: string[], privPackages: string[]) {
  const systemCsv = systemPackages.join(',')
  const privCsv = privPackages.join(',')

  await runOkrmng(
    `replace --system ${shellQuote(systemCsv)} --priv ${shellQuote(privCsv)}`,
    REPLACE_TIMEOUT_MS,
  )
}

export async function getPackagesInfo(packageNames: string[]) {
  if (packageNames.length === 0) {
    return [] as PackageInfoSummary[]
  }

  const bridge = getBridge()
  if (bridge?.getPackagesInfo) {
    try {
      const payload = await Promise.resolve(bridge.getPackagesInfo(packageNames))
      return parsePackageInfoList(payload)
    } catch {
      try {
        const payload = await Promise.resolve(bridge.getPackagesInfo(JSON.stringify(packageNames)))
        return parsePackageInfoList(payload)
      } catch {
        // Fall through to WebUIX-specific paths below.
      }
    }
  }

  const wxuPackageManager = getWxuPackageManager()
  if (wxuPackageManager?.getApplicationInfo) {
    const details = packageNames
      .map((packageName) => {
        try {
          const info = wxuPackageManager.getApplicationInfo?.(packageName, 0, SYSTEM_USER_ID)
          const iconBase64 = wxuPackageManager.getApplicationIcon?.(packageName, 0, SYSTEM_USER_ID)

          return normalizePackageInfoSummary(info, packageName, base64ToDataUrl(iconBase64))
        } catch {
          return null
        }
      })
      .filter((detail): detail is PackageInfoSummary => detail !== null)

    if (details.length > 0) {
      return details
    }
  }

  if (getRuntimeCapabilities().runtime === 'webuix') {
    const details = (
      await Promise.all(
        packageNames.map(async (packageName) => {
          const info = await fetchJsonWithTimeout(
            resolveRuntimeAssetUrl(`.package/${encodeURIComponent(packageName)}/info.json`),
            URL_FETCH_TIMEOUT_MS,
          )

          return normalizePackageInfoSummary(info, packageName, getWebUiXPackageIconUrl(packageName))
        }),
      )
    ).filter((detail): detail is PackageInfoSummary => detail !== null)

    if (details.length > 0) {
      return details
    }
  }

  return null
}

export function getPackageIconUrl(packageName: string) {
  const capabilities = getRuntimeCapabilities()
  if (!capabilities.hasListPackages) {
    return null
  }

  if (capabilities.runtime === 'kernelsu') {
    return `ksu://icon/${encodeURIComponent(packageName)}`
  }

  if (capabilities.runtime === 'webuix') {
    return getWebUiXPackageIconUrl(packageName)
  }

  return null
}

export function showNativeToast(message: string) {
  const bridge = getBridge()
  if (!bridge?.toast) {
    return
  }

  try {
    bridge.toast(message)
  } catch {
    // Ignore native toast failures and let the in-page toast handle it.
  }
}

export function exitWebUi() {
  const bridge = getBridge()
  if (!bridge?.exit) {
    return false
  }

  try {
    bridge.exit()
    return true
  } catch {
    return false
  }
}

export function formatBridgeError(error: unknown) {
  return errorToMessage(error)
}
