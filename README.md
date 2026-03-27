# 📱 OukaroManager

[![Build Status](https://github.com/OukaroMF/OukaroManager/workflows/Build%20KernelSU%20Module/badge.svg)](https://github.com/OukaroMF/OukaroManager/actions)
[![License: Anti-996](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/kattgu7/Anti-996-License)
[![KernelSU](https://img.shields.io/badge/KernelSU-Compatible-green.svg)](https://github.com/tiann/KernelSU)
[![WebUIX](https://img.shields.io/badge/WebUIX-Compatible-orange.svg)](https://github.com/MMRLApp/WebUI-X-Portable)

![OukaroManager logo](webui/src/assets/karo.svg)

一个 KernelSU 模块，提供基于 Vue 3 的 WebUI，将主用户（system user / user 0）的普通 Android 应用加入系统应用配置中。WebUI 可搜索主用户应用、切换 `System`/`Priv` 模式、保存配置，并在保存后明确提示重启生效。

A KernelSU module with a Vue 3 WebUI for turning regular Android apps installed for the primary Android user (system user / user 0) into system-app config entries. The WebUI lets you search primary-user apps, switch between `System` and `Priv` modes, save config, and then reboot to apply.

## ✨ 功能特性 | Features

- 🧱 **将普通应用转换为系统应用** | **Convert regular apps to system apps**
- 📁 **支持 `System` 和 `Priv` 两种模式** | **Supports both `System` and `Priv` modes**
- 🌐 **Vue 3 + shadcn 风格 WebUI** — 在 KernelSU Manager、MMRL 或 WebUIX portable 中使用 | **Vue 3 + shadcn-style WebUI** — Use it in KernelSU Manager, MMRL or WebUIX portable
- 🛠️ **通过模块内 overlayfs 挂载工作**，无需手动重新挂载/system | **Works through module-managed overlayfs mounts**, no manual /system remounting required
- 🔎 **应用搜索与模式切换** — 按包名筛选并逐个选择 `None` / `System` / `Priv` | **Search and mode switching** — Filter by package name and choose `None` / `System` / `Priv`
- 🌍 **多语言支持** — 支持简体中文和英文，并默认跟随浏览器/管理器语言 | **Multi-language support** — Simplified Chinese and English with locale-aware default selection

## 📦 工作原理 | How It Works

该模块会在下一次启动的 KernelSU `post-mount` 阶段，通过模块内的 overlayfs 上层目录，将选定主用户应用同步到 `/system/app` 或 `/system/priv-app`，尽量让它们以“预装应用”的方式被系统扫描。

This module syncs selected primary-user apps into `/system/app` or `/system/priv-app` during the next boot's KernelSU `post-mount` stage by using module-managed overlayfs upper directories, so Android can scan them like pre-installed apps.

## 🚀 安装 | Installation

1. **下载** 最新版本的模块 | **Download** the latest version of the module
2. **安装** 使用KernelSU Manager | **Install** using KernelSU Manager
3. **重启** 您的设备 | **Reboot** your device
4. **访问** 通过KernelSU Manager、MMRL或WebUIX portable访问WebUI | **Access** WebUI via KernelSU Manager, MMRL or WebUIX portable

## 🖥️ 使用方法 | Usage

### WebUI
1. 打开KernelSU Manager（如果KernelSU Manager不可用，可使用MMRL/WebUIX portable） | Open KernelSU Manager (if KernelSU Manager is unavailable, use MMRL/WebUIX portable)
2. 导航到OukaroManager模块WebUI | Navigate to OukaroManager module WebUI
3. 搜索要处理的主用户应用 | Search for the primary-user apps you want to manage
4. 为每个应用选择 `None`、`System` 或 `Priv` | Choose `None`, `System`, or `Priv` for each app
5. 点击保存配置，并在提示后重启设备 | Save the configuration, then reboot when prompted

### 手动修改 | Manual 
> 配置文件路径在 /data/adb/modules/oukaro_manager/config.toml

e.g
```toml
[app]
system_app = ["bin.mt.plus"]
priv_app = ["com.termux"]
```

> 注意 `system_app` 和 `priv_app` 仅能使用应用包名

### CLI
> 模块内会同时打包 `okrmng`，用于 WebUI 和命令行配置管理

```sh
okrmng inspect --json
okrmng replace --system "bin.mt.plus" --priv "com.termux"
```

## ⚠️ 系统要求 | System Requirements

- **KernelSU** 已安装并正常工作 | **KernelSU** installed and working
- **Android设备** 具有root权限 | **Android device** with root access
- **KernelSU Manager**（推荐）或 **MMRL**/**WebUIX portable** 用于WebUI访问 | **KernelSU Manager** (recommended) or **MMRL**/**WebUIX portable** for WebUI access

## 🔧 技术细节 | Technical Details

- 使用模块内 overlayfs 挂载 | Uses module-managed overlayfs mounts
- 无直接系统分区修改 | No direct system partition modifications
- WebUI 保存只会更新 `config.toml`，需要重启后由模块在 `post-mount` 阶段应用挂载 | Saving in WebUI only updates `config.toml`; the module applies mounts during the next boot's `post-mount` stage
- WebUI 和 `okrmng inspect --json` 的“已安装用户应用”语义固定为主用户（system user / user 0），避免多用户/工作资料夹环境下的范围歧义 | The "installed user apps" view in WebUI and `okrmng inspect --json` is intentionally scoped to the primary Android user (system user / user 0) to avoid ambiguity on multi-user and work-profile devices
- 兼容性以“尽量适配”为目标，具体表现仍取决于 Android 版本、ROM 策略和权限模型 | Compatibility is best-effort and still depends on the Android version, ROM policy, and permission model
- **WebUIX兼容**，可增强模块管理体验 | **WebUIX compatible** for enhanced module management experience
- WebUI 使用 `okrmng inspect --json` 读取状态，使用 `okrmng replace` 原子写回配置 | The WebUI reads state through `okrmng inspect --json` and writes config atomically with `okrmng replace`

## 📱 WebUI访问选项 | WebUI Access Options

该模块支持 **WebUIX** 标准，可通过多种方式访问：
This module supports the **WebUIX** standard and can be accessed through multiple ways:

### 主要方式（推荐） | Primary Method (Recommended)
- **KernelSU Manager** - 内置对KernelSU模块的WebUI支持 | Built-in WebUI support for KernelSU modules

### 替代方式 | Alternative Options
- **MMRL** - 面具模块仓库加载器，支持WebUI | Magisk Module Repo Loader with WebUI support
- **WebUIX Portable** - 独立的WebUI查看器 | Standalone WebUI viewer

## 🌍 支持的语言 | Supported Languages

- **English** - 完整支持 | Full support
- **简体中文** - 完整支持 | Full support


## 🔄 转换模式 | Conversion Modes

### System: `/system/app/`
标准系统应用位置。适合希望在下次开机时以“预装应用”方式参与系统扫描的大多数普通应用。
Standard system app location. Suitable for most regular apps that need to be scanned like pre-installed apps on the next boot.

### Priv: `/system/priv-app/`
特权系统应用位置，但这不等于现代 Android 一定授予特权权限。自 Android 8.0/9 起，很多 ROM 还要求同分区的 `privapp-permissions.xml` allowlist；本模块不会自动生成这些 XML。
Privileged system app location, but this does not guarantee privileged permissions on modern Android. Since Android 8.0/9, many ROMs still require same-partition `privapp-permissions.xml` allowlists, and this module does not generate those XML files automatically.

## 🛡️ 安全说明 | Security Notes

- 转换应用为系统应用会赋予它们额外的权限 | Converting apps to system apps grants them additional permissions
- 请仅转换您信任的应用 | Only convert apps you trust
- 备份重要数据，以防意外情况 | Backup important data in case of unexpected issues
- `Priv` 模式在严格执行 privileged-permission allowlist 的 ROM 上可能无法达到预期，极端情况下还可能引发启动期兼容性问题 | `Priv` mode may not behave as expected on ROMs that strictly enforce privileged-permission allowlists, and in extreme cases can cause boot-time compatibility issues
- 可以随时通过 WebUI 将应用改回 `None`、保存配置并重启来还原更改 | You can revert changes anytime by switching an app back to `None` in WebUI, saving, and rebooting

## 🐛 故障排除 | Troubleshooting

### WebUI无法访问 | WebUI Not Accessible
1. 确保KernelSU Manager已更新到最新版本 | Ensure KernelSU Manager is updated to the latest version
2. 尝试使用MMRL或WebUIX portable作为替代 | Try using MMRL or WebUIX portable as alternatives
3. 检查模块是否正确安装并启用 | Check if the module is properly installed and enabled

### 应用转换失败 | App Conversion Fails
1. 确保有足够的存储空间 | Ensure sufficient storage space
2. 检查应用是否已经是系统应用 | Check if the app is already a system app
3. 确认已经点击“保存配置”，然后重启设备再检查结果 | Make sure you saved the config, then reboot the device and check again

### 转换后应用无法正常工作 | Apps Not Working After Conversion
1. 在 WebUI 中将应用切回 `None`，保存后重启 | Switch the app back to `None` in WebUI, save, and reboot
2. 清除应用数据和缓存 | Clear app data and cache
3. 检查应用是否与您的Android版本兼容 | Check if the app is compatible with your Android version

### WebUI显示失效配置 | WebUI Shows Stale Configuration
1. 这表示某些已配置包名不再属于当前主用户应用列表 | This means some configured package names are no longer present for the primary Android user
2. WebUI 保存时会保留这些条目，不会自动丢失 | The WebUI preserves those entries when saving
3. 如需移除，请通过 WebUI 重新选择有效包，或手动编辑 `config.toml` | Remove them by selecting valid packages in WebUI or editing `config.toml` manually

## 🤝 贡献 | Contributing

欢迎贡献代码、报告问题或提出改进建议！
We welcome contributions! Please feel free to:
- 报告错误和问题 | Report bugs and issues
- 建议新功能 | Suggest new features
- 提交拉取请求 | Submit pull requests
- 改进文档 | Improve documentation

## 🕸 一些链接 | Some Links

- **GitHub**: [OukaroMF/OukaroManager](https://github.com/OukaroMF/OukaroManager)
- **Telegram**: [@MF_1f1e33](https://t.me/MF_1f1e33) | [@OukaroSU](https://t.me/OukaroSU) | [@MFnotMtF](https://t.me/MFnotMtF)

## 🙏 致谢 | Acknowledgements

- **[KOWX712](https://github.com/KOWX712)** - [Tricky-Addon](https://github.com/KOWX712/Tricky-Addon-Update-Target-List) 项目提供的优化灵感和应用列表管理技术
- **[KernelSU](https://github.com/tiann/KernelSU)** - 提供强大的内核级 root 解决方案
- **WebUIX** - 提供 WebUI 兼容性支持
- **妳** - 所有用户和贡献者，感谢您的支持和反馈！

Special thanks to:
- **[KOWX712](https://github.com/KOWX712)** - For optimization inspiration and app list management techniques from [Tricky-Addon](https://github.com/KOWX712/Tricky-Addon-Update-Target-List)
- **[KernelSU](https://github.com/tiann/KernelSU)** - For providing powerful kernel-level root solution
- **WebUIX** - For WebUI compatibility support
- **You** - All users and contributors, thank you for your support and feedback!

## ⚠️ 免责声明 | Disclaimer

- 此模块会修改系统行为 - 使用风险自负 | This module modifies system behavior - use at your own risk
- 使用前请务必备份设备 | Always backup your device before using
- 某些应用可能无法作为系统应用正常工作 | Some apps may not function properly as system apps
- 我们不对设备的任何损坏承担责任 | We are not responsible for any damage to your device

---

⭐ 如果这个项目对您有帮助，请给我们一个星标！ | If this project helps you, please give us a star!

---

## 📜 许可证 | License

本项目采用 **Anti-996 License Version 1.0** 许可证。
This project is licensed under the **Anti-996 License Version 1.0**.

[![Anti 996](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/kattgu7/Anti-996-License)

### 关于 Anti-996 License | About Anti-996 License

Anti-996 License 是一个开源许可证，旨在保护开发者的劳动权益，反对过度加班的"996"工作制度（上午9点到晚上9点，一周6天）。

The Anti-996 License is an open source license designed to protect developers' labor rights and oppose the excessive overtime "996" work system (9 AM to 9 PM, 6 days a week).

### 主要条款 | Main Terms

此许可证的主要条款包括：
The main terms of this license include:

1. **许可授予** | **License Grant**：允许任何个人或法律实体免费使用、复制、修改、发布和分发本许可作品 | Allows any individual or legal entity to freely use, copy, modify, publish and distribute this licensed work
2. **劳动法合规要求** | **Labor Law Compliance**：使用者必须严格遵守所在司法管辖区的所有相关劳动和就业法律法规 | Users must strictly comply with all relevant labor and employment laws and regulations in their jurisdiction
3. **员工权益保护** | **Employee Rights Protection**：禁止以任何方式诱导或强迫员工放弃其劳动权益 | Prohibits inducing or forcing employees to give up their labor rights in any way

### 为什么选择 Anti-996 License？ | Why Anti-996?

- ✊ **保护开发者权益** | **Protect Developer Rights**：确保使用本软件的公司遵守合理的工作时间 | Ensure companies using this software comply with reasonable working hours
- 🌟 **促进健康工作环境** | **Promote Healthy Work Environment**：反对过度加班，提倡工作与生活的平衡 | Oppose excessive overtime and advocate work-life balance
- 🔒 **法律约束力** | **Legal Binding**：通过许可证条款确保劳动法的遵守 | Ensure compliance with labor laws through license terms

### 了解更多 | Learn More

- [Anti-996 License 项目 | Anti-996 License Project](https://github.com/kattgu7/Anti-996-License)
- [996.ICU](https://996.icu/)

---

**注意** | **Notice**：通过使用本项目，您同意遵守 Anti-996 License 的所有条款和条件。 | By using this project, you agree to comply with all terms and conditions of the Anti-996 License.
