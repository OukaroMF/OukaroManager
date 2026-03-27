export type AppMode = 'none' | 'system' | 'priv'
export type ModuleRuntime = 'preview' | 'kernelsu' | 'webuix'
export type InspectUserAppsSource =
  | 'pmListPackages'
  | 'packagesXmlAndRestrictions'
  | 'packagesXmlBestEffort'
export type InspectSystemUserStateSource = 'packageRestrictions'

export interface InspectPayload {
  systemApp: string[]
  privApp: string[]
  installedUserApps: string[]
  missingConfiguredApps: string[]
  installedUserAppsSource?: InspectUserAppsSource
  systemUserStateSource?: InspectSystemUserStateSource
  warnings?: string[]
}

export interface ModuleMetadata {
  moduleDir: string
  id?: string
  name?: string
  version?: string
  versionCode?: string
  description?: string
}

export interface RuntimeCapabilities {
  runtime: ModuleRuntime
  hasBridge: boolean
  hasExec: boolean
  hasModuleInfo: boolean
  hasWxuModule: boolean
  hasWxuPackageManager: boolean
  hasToast: boolean
  hasEdgeToEdge: boolean
  hasListPackages: boolean
  hasPackageInfo: boolean
  hasExit: boolean
}

export interface PackageInfoSummary {
  packageName: string
  versionName?: string
  versionCode?: number
  appLabel?: string
  iconUrl?: string | null
  isSystem?: boolean | null
  uid?: number | null
  error?: string
}
