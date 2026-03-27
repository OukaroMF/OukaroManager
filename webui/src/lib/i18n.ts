import { createI18n } from 'vue-i18n'

const LOCALE_STORAGE_KEY = 'oukaro.webui.locale'

const messages = {
  en: {
    title: 'System App Workbench',
    subtitle:
      'Choose apps installed for the primary Android user (user 0), place them into System or Priv mode, and save the module config for the next reboot.',
    language: {
      label: 'Language',
      zh: '中文',
      en: 'English',
    },
    header: {
      eyebrow: 'System app configuration console',
      supported: 'KernelSU linked',
      preview: 'Preview only',
      runtime: 'Runtime',
      runtimePreview: 'Preview browser',
      runtimeKernelSu: 'KernelSU Manager',
      runtimeWebUiX: 'WebUIX',
      capabilityFull: 'Exec and module metadata available',
      capabilityLimited: 'Bridge is limited in this environment',
      reboot: 'Reboot required after save',
      managedBy: 'Backed by okrmng inspect/replace',
      modulePath: 'Module path',
      moduleId: 'Module ID',
      version: 'Version',
      modulePending: 'Waiting for module metadata',
      moduleUnavailable: 'Module metadata unavailable in preview mode',
    },
    actions: {
      save: 'Save configuration',
      saving: 'Saving configuration...',
      reset: 'Reset draft',
      refresh: 'Reload state',
      showMore: 'Show all',
      showLess: 'Show less',
    },
    list: {
      title: 'Installed primary-user apps',
      description:
        'Search by package name and assign exactly one target mode per app for Android user 0.',
      searchLabel: 'Search packages',
      searchPlaceholder: 'com.example.app',
      modeLabel: 'Target mode',
      detailsLoading: 'Loading package details',
      detailsReady: 'Package details ready',
      limitedCount: 'Showing {shown} of {total} matches',
      empty: 'No installed primary-user apps matched the current search.',
      noData: 'okrmng did not return any primary-user apps.',
      packageCount: '{count} apps available',
      sourceLabel: 'Discovery source: {source}',
    },
    summary: {
      title: 'Current draft',
      description:
        'Counts cover the whole saved config, including preserved stale entries.',
      installed: 'Primary-user apps',
      configured: 'Configured apps',
      system: 'System',
      priv: 'Privileged',
      none: 'Unset',
      stale: 'Missing configured apps',
      synced: 'Config matches disk',
      dirty: 'Unsaved changes',
      savedAt: 'Last saved at {time}',
    },
    mode: {
      none: 'None',
      system: 'System',
      priv: 'Priv',
      noneHint: 'Keep this package out of the module config.',
      systemHint: 'Mount this package under /system/app on next reboot.',
      privHint: 'Mount this package under /system/priv-app on next reboot.',
    },
    alerts: {
      unsupportedTitle: 'KernelSU APIs are unavailable',
      unsupportedBody:
        'This page can render in a normal browser for layout work, but refresh and save stay disabled until it runs inside KernelSU Manager or WebUIX.',
      limitedBridgeTitle: 'Bridge support is partial',
      limitedBridgeBody:
        'This runtime exposed only part of the KernelSU bridge. Loading or saving may stay unavailable until both exec and moduleInfo are present.',
      loadFailedTitle: 'Could not load module state',
      saveFailedTitle: 'Could not save configuration',
      inspectFallbackTitle: 'Android package discovery is in fallback mode',
      inspectFallbackBody:
        'Installed app discovery is using {source}. {details}',
      inspectFallbackDefault:
        'The package list was reconstructed from Android package settings metadata instead of `pm list packages`.',
      missingTitle: 'Stale configuration preserved',
      missingBody:
        '{count} configured packages are no longer listed for the primary Android user. Saving keeps those stale entries unchanged.',
      privTitle: 'Priv mode remains ROM-dependent',
      privBody:
        'Mounting under /system/priv-app does not automatically grant privileged permissions on Android 8.0+. Many ROMs still require same-partition privapp-permissions XML allowlists.',
      rebootTitle: 'Saving only updates config.toml',
      rebootBody:
        'The module applies saved mounts during the next boot\'s post-mount stage. Save your draft, then reboot the device to activate it.',
    },
    toasts: {
      saved: 'Configuration saved. Reboot to apply.',
      saveFailed: 'Could not save configuration.',
      refreshed: 'Module state refreshed.',
      loadFailed: 'Could not load module state.',
      draftRestored: 'Restored an unsaved draft from the last session.',
    },
    status: {
      loading: 'Loading module state...',
      refreshing: 'Refreshing...',
      saving: 'Saving...',
    },
    labels: {
      current: 'Current mode',
      changed: 'Changed',
      preserved: 'Preserved',
      selected: 'Selected',
    },
    sources: {
      pmListPackages: 'pm list packages',
      packagesXmlAndRestrictions: 'packages.xml + package-restrictions.xml',
      packagesXmlBestEffort: 'packages.xml best effort',
    },
  },
  'zh-CN': {
    title: '系统应用工作台',
    subtitle:
      '选择主用户（user 0）已安装的应用，将它们切到 System 或 Priv 模式，并保存模块配置，等待下次重启生效。',
    language: {
      label: '语言',
      zh: '中文',
      en: 'English',
    },
    header: {
      eyebrow: '系统应用配置控制台',
      supported: '已连接 KernelSU',
      preview: '仅预览模式',
      runtime: '运行环境',
      runtimePreview: '预览浏览器',
      runtimeKernelSu: 'KernelSU Manager',
      runtimeWebUiX: 'WebUIX',
      capabilityFull: '已具备 exec 与 moduleInfo 能力',
      capabilityLimited: '当前环境桥接能力不完整',
      reboot: '保存后需要重启',
      managedBy: '由 okrmng inspect/replace 驱动',
      modulePath: '模块路径',
      moduleId: '模块 ID',
      version: '版本',
      modulePending: '正在等待模块元数据',
      moduleUnavailable: '预览模式下无法读取模块元数据',
    },
    actions: {
      save: '保存配置',
      saving: '正在保存配置...',
      reset: '重置草稿',
      refresh: '重新读取状态',
      showMore: '展开全部',
      showLess: '收起列表',
    },
    list: {
      title: '主用户已安装应用',
      description: '按包名搜索，并仅针对主用户（user 0）为每个应用选择一个目标模式。',
      searchLabel: '搜索包名',
      searchPlaceholder: 'com.example.app',
      modeLabel: '目标模式',
      detailsLoading: '正在读取应用详情',
      detailsReady: '应用详情已就绪',
      limitedCount: '当前显示 {shown} / {total} 条匹配结果',
      empty: '当前搜索条件下没有匹配到主用户应用。',
      noData: 'okrmng 没有返回任何主用户应用。',
      packageCount: '共 {count} 个应用',
      sourceLabel: '发现来源：{source}',
    },
    summary: {
      title: '当前草稿',
      description: '统计覆盖整个保存配置，包含仍被保留的失效条目。',
      installed: '主用户应用数',
      configured: '已配置应用',
      system: 'System',
      priv: 'Priv',
      none: '未设置',
      stale: '失效配置条目',
      synced: '当前配置已与磁盘一致',
      dirty: '有未保存更改',
      savedAt: '最近保存于 {time}',
    },
    mode: {
      none: 'None',
      system: 'System',
      priv: 'Priv',
      noneHint: '不把这个应用写进模块配置。',
      systemHint: '下次重启后把它挂载到 /system/app。',
      privHint: '下次重启后把它挂载到 /system/priv-app。',
    },
    alerts: {
      unsupportedTitle: '当前环境没有 KernelSU API',
      unsupportedBody:
        '这个页面可以在普通浏览器里预览布局，但刷新和保存功能只有在 KernelSU Manager 或 WebUIX 里运行时才可用。',
      limitedBridgeTitle: '桥接能力不完整',
      limitedBridgeBody:
        '当前运行环境只暴露了部分 KernelSU 桥接接口。只有当 exec 和 moduleInfo 都可用时，读取和保存才能稳定工作。',
      loadFailedTitle: '读取模块状态失败',
      saveFailedTitle: '保存配置失败',
      inspectFallbackTitle: 'Android 包发现当前处于回退模式',
      inspectFallbackBody:
        '当前已安装应用列表使用 {source} 得出。{details}',
      inspectFallbackDefault:
        '当前列表并非来自 `pm list packages`，而是由 Android 包设置元数据重建得到。',
      missingTitle: '已保留失效配置',
      missingBody:
        '有 {count} 个已配置包名不再属于主用户（user 0）应用列表。保存时会保留这些失效条目，不会自动丢失。',
      privTitle: 'Priv 模式仍然依赖 ROM 实现',
      privBody:
        '把应用挂到 /system/priv-app 并不等于 Android 8.0+ 一定授予特权权限。很多 ROM 仍要求同分区的 privapp-permissions XML allowlist。',
      rebootTitle: '保存只会更新 config.toml',
      rebootBody:
        '模块会在下一次开机的 post-mount 阶段应用这些挂载。先保存草稿，再重启设备让改动生效。',
    },
    toasts: {
      saved: '配置已保存，请重启后生效。',
      saveFailed: '保存配置失败。',
      refreshed: '模块状态已刷新。',
      loadFailed: '读取模块状态失败。',
      draftRestored: '已恢复上次未保存的草稿。',
    },
    status: {
      loading: '正在读取模块状态...',
      refreshing: '正在刷新...',
      saving: '正在保存...',
    },
    labels: {
      current: '当前模式',
      changed: '已变更',
      preserved: '已保留',
      selected: '已选择',
    },
    sources: {
      pmListPackages: 'pm list packages',
      packagesXmlAndRestrictions: 'packages.xml + package-restrictions.xml',
      packagesXmlBestEffort: 'packages.xml 尽力推断',
    },
  },
} as const

function detectInitialLocale() {
  try {
    const savedLocale = window.localStorage.getItem(LOCALE_STORAGE_KEY)
    if (savedLocale === 'zh-CN' || savedLocale === 'en') {
      return savedLocale
    }
  } catch {
    // Ignore storage access failures and fall back to navigator.language.
  }

  return navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en'
}

export const i18n = createI18n({
  legacy: false,
  locale: detectInitialLocale(),
  fallbackLocale: 'en',
  messages,
})

export function persistLocale(nextLocale: string) {
  try {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, nextLocale)
  } catch {
    // Ignore storage failures in restricted WebView environments.
  }
}
