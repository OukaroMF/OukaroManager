import { enableEdgeToEdge, exec, moduleInfo, toast as kernelToast } from 'kernelsu'

import type { InspectPayload, ModuleMetadata } from '@/lib/types'

declare global {
  interface Window {
    ksu?: unknown
    kernelsu?: unknown
  }
}

const OKRMNG_COMMAND = './okrmng'

let moduleMetadataPromise: Promise<ModuleMetadata> | null = null

function ensureKernelSuAlias() {
  if (typeof window === 'undefined') {
    return
  }

  if (typeof window.ksu === 'undefined' && typeof window.kernelsu !== 'undefined') {
    window.ksu = window.kernelsu
  }
}

function shellQuote(value: string) {
  return `'${value.replace(/'/g, `'\"'\"'`)}'`
}

function commandError(stderr: string, stdout: string, errno: number) {
  const message = stderr.trim() || stdout.trim()
  return new Error(message || `Command failed with exit code ${errno}`)
}

function parseModuleMetadata(raw: string): ModuleMetadata {
  const parsed = JSON.parse(raw) as ModuleMetadata

  if (!parsed.moduleDir) {
    throw new Error('KernelSU did not provide a moduleDir value.')
  }

  return parsed
}

async function runOkrmng(argumentsText: string) {
  const metadata = await getModuleMetadata()

  if (!metadata) {
    throw new Error('KernelSU WebUI APIs are unavailable in this environment.')
  }

  const result = await exec(`${OKRMNG_COMMAND} ${argumentsText}`, {
    cwd: metadata.moduleDir,
  })

  if (result.errno !== 0) {
    throw commandError(result.stderr, result.stdout, result.errno)
  }

  return result.stdout.trim()
}

export function isKernelSuAvailable() {
  if (typeof window === 'undefined') {
    return false
  }

  ensureKernelSuAlias()
  return typeof window.ksu !== 'undefined'
}

export function requestEdgeToEdge() {
  if (!isKernelSuAvailable()) {
    return
  }

  try {
    enableEdgeToEdge(true)
  } catch {
    // Ignore optional runtime helpers when previewing in non-KernelSU environments.
  }
}

export async function getModuleMetadata() {
  if (!isKernelSuAvailable()) {
    return null
  }

  if (!moduleMetadataPromise) {
    moduleMetadataPromise = Promise.resolve()
      .then(() => parseModuleMetadata(moduleInfo()))
      .catch((error) => {
        moduleMetadataPromise = null
        throw error
      })
  }

  return moduleMetadataPromise
}

export async function inspectConfig() {
  const stdout = await runOkrmng('inspect --json')
  return JSON.parse(stdout) as InspectPayload
}

export async function replaceConfig(systemPackages: string[], privPackages: string[]) {
  const systemCsv = systemPackages.join(',')
  const privCsv = privPackages.join(',')

  await runOkrmng(
    `replace --system ${shellQuote(systemCsv)} --priv ${shellQuote(privCsv)}`,
  )
}

export function showNativeToast(message: string) {
  if (!isKernelSuAvailable()) {
    return
  }

  try {
    kernelToast(message)
  } catch {
    // Ignore native toast failures and let the in-page toast handle it.
  }
}
