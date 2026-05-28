# Z-Library NoProxy

[![CI](https://github.com/EliorFoy/zlibrary-tauri/actions/workflows/ci.yml/badge.svg)](https://github.com/EliorFoy/zlibrary-tauri/actions/workflows/ci.yml)
[![Build and Release](https://github.com/EliorFoy/zlibrary-tauri/actions/workflows/release.yml/badge.svg)](https://github.com/EliorFoy/zlibrary-tauri/actions/workflows/release.yml)

Z-Library NoProxy 是一个基于 Tauri 2、Vue 3 和 Rust 构建的 Z-Library 客户端，目标是在桌面端提供无需代理的搜索、账号管理和下载体验，同时保留 CLI 工具用于诊断和自动化。

> 本项目仅供学习研究使用。请遵守当地法律法规，支持正版图书。

## 功能

- 无需代理访问 Z-Library。
- 桌面 GUI：支持 Windows、macOS、Linux。
- CLI 工具：包含搜索、下载、诊断、挑战调试等命令。
- Android APK：通过 GitHub Actions 构建。
- 下载管理：显示下载进度、保存路径，并可打开文件所在位置。
- 账号池：支持登录、注册、额度刷新和自动选择可用账号。
- 跨平台运行文件：数据库、日志、缓存使用系统应用数据目录，避免 AppImage 等环境写入可执行文件目录。

## 下载

正式版本会发布在 [GitHub Releases](https://github.com/EliorFoy/zlibrary-tauri/releases)。

Release 页面通常包含：

| 类型 | 平台 | 文件 |
| --- | --- | --- |
| GUI | Windows | `Z-Library-NoProxy-GUI-Windows-x64-MSI.msi` / `Z-Library-NoProxy-GUI-Windows-x64-Setup.exe` |
| GUI | macOS | `Z-Library-NoProxy-GUI-macOS-*-DMG.dmg` |
| GUI | Linux | `Z-Library-NoProxy-GUI-Linux-x64-AppImage.AppImage` / `.deb` / `.rpm` |
| CLI | Windows | `Z-Library-NoProxy-CLI-Windows-x64.zip` |
| CLI | macOS | `Z-Library-NoProxy-CLI-macOS-*.tar.gz` |
| CLI | Linux | `Z-Library-NoProxy-CLI-Linux-x64.tar.gz` |
| Android | APK | `Z-Library-NoProxy-Android-*` |

iOS 暂未支持发布。iOS 需要单独的 Tauri iOS 初始化、Apple 签名证书和 provisioning profile。

## 开发环境

需要安装：

- Node.js 20 或更高版本
- Rust stable
- npm
- Tauri 2 对应平台依赖

Linux 额外依赖，以 Ubuntu/Debian 为例：

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  build-essential \
  curl \
  wget \
  file
```

Android 构建还需要：

- Java 17
- Android SDK
- Android NDK `26.1.10909125`
- Rust Android targets

## 本地开发

```bash
git clone https://github.com/EliorFoy/zlibrary-tauri.git
cd zlibrary-tauri
npm install
npm run tauri dev
```

前端开发服务器：

```bash
npm run dev
```

## 本地检查

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml --features gui
cargo check --manifest-path src-tauri/Cargo.toml --features cli
```

Windows PowerShell 下也可以使用反斜杠路径：

```powershell
npm run build
cargo check --manifest-path src-tauri\Cargo.toml --features gui
cargo check --manifest-path src-tauri\Cargo.toml --features cli
```

## 构建

构建当前平台 GUI：

```bash
npm run tauri build
```

产物位于：

```text
src-tauri/target/release/bundle/
```

构建 CLI：

```bash
cd src-tauri
cargo build --release --features cli --bin zlibrary-cli
```

构建 Android APK：

```bash
npm run tauri -- android init --skip-targets-install --ci
npm run tauri -- android build --apk --features gui --ci
```

## 发布

普通 push 到 `master` 或 `main` 会运行 CI 和跨平台构建，并上传 GitHub Actions artifacts。

正式发布通过 tag 触发：

```bash
git tag v0.1.3
git push origin v0.1.3
```

推送 `v*` tag 后，`.github/workflows/release.yml` 会创建或更新 GitHub Release，并上传 GUI、CLI 和 Android APK 产物。

发布前请确认这些版本号一致：

```text
package.json
package-lock.json
src-tauri/Cargo.toml
src-tauri/tauri.conf.json
```

## 项目结构

```text
zlibrary-tauri/
├── .github/workflows/      # CI 和发布 workflow
├── src/                    # Vue 前端
├── src-tauri/              # Tauri / Rust 后端
│   ├── src/                # Rust 源码
│   ├── capabilities/       # Tauri 权限配置
│   ├── icons/              # 应用图标
│   └── tauri.conf.json     # Tauri 配置
├── index.html
├── package.json
├── package-lock.json
├── vite.config.ts
└── README.md
```

## 许可证

MIT License
