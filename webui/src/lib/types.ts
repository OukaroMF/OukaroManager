export type AppMode = 'none' | 'system' | 'priv'

export interface InspectPayload {
  systemApp: string[]
  privApp: string[]
  installedUserApps: string[]
  missingConfiguredApps: string[]
}

export interface ModuleMetadata {
  moduleDir: string
  id?: string
  name?: string
  version?: string
  versionCode?: string
  description?: string
}
