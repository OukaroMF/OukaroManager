# 📱 OukaroManager

[![Build Status](https://github.com/OukaroMF/OukaroManager/workflows/Build%20KernelSU%20Module/badge.svg)](https://github.com/OukaroMF/OukaroManager/actions)
[![License: Anti-996](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/kattgu7/Anti-996-License)
[![KernelSU](https://img.shields.io/badge/KernelSU-Compatible-green.svg)](https://github.com/tiann/KernelSU)
[![WebUIX](https://img.shields.io/badge/WebUIX-Compatible-orange.svg)](https://github.com/MMRLApp/WebUI-X-Portable)

![OukaroManager logo](webui/src/assets/karo.svg)

A KernelSU module that provides a Vue 3 WebUI for adding regular Android apps installed for the primary Android user (system user / user 0) into the system app configuration. The WebUI lets you search primary-user apps, switch between `System` and `Priv` modes, save the configuration, and shows a clear reboot prompt after saving so changes can take effect.

## ✨ Features

- 🧱 **Convert regular apps to system apps**
- 📁 **Supports both `System` and `Priv` modes**
- 🌐 **Vue 3 + shadcn-style WebUI** — Works in KernelSU Manager, MMRL, or WebUIX Portable
- 🛠️ **Works through module-managed overlayfs mounts** — no need to manually remount `/system`
- 🔎 **App search and mode switching** — Filter by package name and choose `None` / `System` / `Priv`
- 🌍 **Multi-language support** — Supports Simplified Chinese and English, with locale-aware default selection

## 📦 How It Works

During the next boot's KernelSU `post-mount` stage, the module uses its own overlayfs upper directories to sync selected primary-user apps into `/system/app` or `/system/priv-app`, so Android can scan them as if they were pre-installed apps.

## 🚀 Installation

1. **Download** the latest version of the module
2. **Install** it using KernelSU Manager
3. **Reboot** your device
4. **Access** the WebUI through KernelSU Manager, MMRL, or WebUIX Portable

## 🖥️ Usage

### WebUI
1. Open KernelSU Manager (if KernelSU Manager is unavailable, use MMRL or WebUIX Portable)
2. Navigate to the OukaroManager module WebUI
3. Search for the primary-user apps you want to manage
4. Choose `None`, `System`, or `Priv` for each app
5. Save the configuration, then reboot the device when prompted

### Manual
> The configuration file is located at `/data/adb/modules/oukaro_manager/config.toml`

Example:
```toml
[app]
system_app = ["bin.mt.plus"]
priv_app = ["com.termux"]
```

> Note: `system_app` and `priv_app` can only contain app package names.

### CLI
> The module also ships with `okrmng` for both WebUI and command-line configuration management.

```sh
okrmng inspect --json
okrmng replace --system "bin.mt.plus" --priv "com.termux"
```

## ⚠️ System Requirements

- **KernelSU** installed and working properly
- **Android device** with root access
- **KernelSU Manager** (recommended), **MMRL**, or **WebUIX Portable** for WebUI access

## 🔧 Technical Details

- Uses module-managed overlayfs mounts
- No direct system partition modification
- Saving in the WebUI only updates `config.toml`; the module applies mounts during the next boot's `post-mount` stage
- The “installed user apps” view in the WebUI and `okrmng inspect --json` is intentionally scoped to the primary Android user (system user / user 0) to avoid ambiguity on multi-user and work-profile devices
- The WebUI build includes `webroot/config.json` to enable `/.package/...` icon and info resource fetching on compatible WebUIX hosts
- Compatibility is best-effort and still depends on the Android version, ROM policy, and permission model
- **WebUIX compatible** for an enhanced module management experience
- The WebUI reads state through `okrmng inspect --json` and writes config atomically with `okrmng replace`

## 📱 WebUI Access Options

This module supports the **WebUIX** standard and can be accessed in multiple ways:

### Primary Method (Recommended)
- **KernelSU Manager** — Built-in WebUI support for KernelSU modules

### Alternative Options
- **MMRL** — Magisk Module Repo Loader with WebUI support
- **WebUIX Portable** — Standalone WebUI viewer

## 🌍 Supported Languages

- **English** — Full support
- **Simplified Chinese** — Full support

## 🔄 Conversion Modes

### System: `/system/app/`
Standard system app location. Suitable for most regular apps that should participate in system scanning like pre-installed apps on the next boot.

### Priv: `/system/priv-app/`
Privileged system app location, but this does not guarantee privileged permissions on modern Android. Since Android 8.0/9, many ROMs also require same-partition `privapp-permissions.xml` allowlists, and this module does not generate those XML files automatically.

## 🛡️ Security Notes

- Converting apps to system apps grants them additional privileges
- Only convert apps you trust
- Back up important data in case of unexpected issues
- `Priv` mode may not behave as expected on ROMs that strictly enforce privileged-permission allowlists, and in extreme cases it may cause boot-time compatibility issues
- You can revert changes at any time by switching an app back to `None` in the WebUI, saving, and rebooting

## 🐛 Troubleshooting

### WebUI Not Accessible
1. Make sure KernelSU Manager is updated to the latest version
2. Try using MMRL or WebUIX Portable as an alternative
3. Check whether the module is properly installed and enabled

### App Conversion Fails
1. Make sure there is enough storage space
2. Check whether the app is already a system app
3. Make sure you saved the configuration, then reboot the device and check the result again

### Apps Not Working After Conversion
1. Switch the app back to `None` in the WebUI, save, and reboot
2. Clear app data and cache
3. Check whether the app is compatible with your Android version

### WebUI Shows Stale Configuration
1. This means some configured package names are no longer present in the current primary-user app list
2. The WebUI preserves those entries when saving and does not remove them automatically
3. To remove them, select valid packages again in the WebUI or edit `config.toml` manually

## 🤝 Contributing

Contributions are welcome! Feel free to:
- Report bugs and issues
- Suggest new features
- Submit pull requests
- Improve documentation

## 🕸 Some Links

- **GitHub**: [OukaroMF/OukaroManager](https://github.com/OukaroMF/OukaroManager)
- **Telegram**: [@MF_1f1e33](https://t.me/MF_1f1e33) | [@OukaroSU](https://t.me/OukaroSU) | [@MFnotMtF](https://t.me/MFnotMtF)

## 🙏 Acknowledgements

Special thanks to:
- **[KOWX712](https://github.com/KOWX712)** — For optimization inspiration and app list management techniques from the [Tricky-Addon](https://github.com/KOWX712/Tricky-Addon-Update-Target-List) project
- **[KernelSU](https://github.com/tiann/KernelSU)** — For providing a powerful kernel-level root solution
- **WebUIX** — For WebUI compatibility support
- **You** — All users and contributors, thank you for your support and feedback!

## ⚠️ Disclaimer

- This module modifies system behavior — use it at your own risk
- Always back up your device before using it
- Some apps may not work properly as system apps
- We are not responsible for any damage to your device

---

⭐ If this project helps you, please give it a star!

---

## 📜 License

This project is licensed under the **Anti-996 License Version 1.0**.

[![Anti 996](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/kattgu7/Anti-996-License)

### About the Anti-996 License

The Anti-996 License is an open-source license designed to protect developers' labor rights and oppose the excessive overtime “996” work system (9 AM to 9 PM, 6 days a week).

### Main Terms

The main terms of this license include:

1. **License Grant** — Allows any individual or legal entity to freely use, copy, modify, publish, and distribute this licensed work
2. **Labor Law Compliance** — Users must strictly comply with all relevant labor and employment laws and regulations in their jurisdiction
3. **Employee Rights Protection** — Prohibits inducing or forcing employees to give up their labor rights in any way

### Why Anti-996?

- ✊ **Protect Developer Rights** — Ensures that companies using this software comply with reasonable working hours
- 🌟 **Promote a Healthy Work Environment** — Opposes excessive overtime and advocates work-life balance
- 🔒 **Legal Binding** — Helps enforce labor-law compliance through license terms

### Learn More

- [Anti-996 License Project](https://github.com/kattgu7/Anti-996-License)
- [996.ICU](https://996.icu/)

---

**Notice**: By using this project, you agree to comply with all terms and conditions of the Anti-996 License.
