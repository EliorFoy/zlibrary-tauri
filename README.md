# Z-Library NoProxy

一个基于 Tauri v2 和 Vue 3 构建的 Z-Library 桌面客户端，无需代理即可访问。

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite
- **桌面框架**: Tauri v2
- **插件**: 
  - `@tauri-apps/plugin-dialog` - 文件对话框支持
  - `@tauri-apps/plugin-fs` - 文件系统操作

## 功能特性

- 🚀 无需代理直接访问 Z-Library
- 📦 跨平台桌面应用 (Windows, macOS, Linux)
- 🎨 现代化的用户界面
- 🔒 本地运行，保护隐私
- 📥 支持文件下载和管理

## 环境要求

在开始之前，请确保你的系统已安装以下依赖：

- [Node.js](https://nodejs.org/) (v18 或更高版本)
- [Rust](https://rustup.rs/) (最新稳定版)
- npm 或 pnpm

### 系统特定依赖

**Windows:**
- Microsoft Visual Studio C++ Build Tools

**macOS:**
- Xcode Command Line Tools

**Linux:**
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev

# Fedora
sudo dnf install -y webkit2gtk4.1-devel build-essential curl wget file xdotool openssl-devel libappindicator-gtk3-devel librsvg2-devel
```

## 安装与开发

### 1. 克隆项目

```bash
git clone <repository-url>
cd zlibrary-tauri
```

### 2. 安装依赖

```bash
npm install
```

### 3. 启动开发服务器

```bash
npm run tauri dev
```

这将同时启动 Vite 开发服务器和 Tauri 应用窗口。

## 构建发布版本

### 构建所有平台

```bash
npm run tauri build
```

构建完成后，安装包将位于 `src-tauri/target/release/bundle/` 目录下。

### 仅构建前端

```bash
npm run build
```

### 预览生产构建

```bash
npm run preview
```

## 可用脚本

| 命令 | 描述 |
|------|------|
| `npm run dev` | 启动 Vite 开发服务器 |
| `npm run build` | 构建前端生产版本 |
| `npm run preview` | 预览生产构建 |
| `npm run tauri` | Tauri CLI 命令 |
| `npm run tauri dev` | 开发模式运行 Tauri 应用 |
| `npm run tauri build` | 构建生产版本的 Tauri 应用 |

## 项目结构

```
zlibrary-tauri/
├── src/              # Vue 前端源代码
├── src-tauri/        # Tauri Rust 后端代码
│   ├── src/          # Rust 源代码
│   ├── icons/        # 应用图标
│   └── tauri.conf.json  # Tauri 配置文件
├── index.html        # HTML 入口
├── package.json      # Node.js 依赖配置
├── tsconfig.json     # TypeScript 配置
├── vite.config.ts    # Vite 配置
└── README.md         # 项目说明文档
```

## 配置说明

应用配置位于 `src-tauri/tauri.conf.json`，主要配置项包括：

- **productName**: 应用名称 (Z-Library NoProxy)
- **version**: 应用版本 (0.1.0)
- **identifier**: 应用唯一标识符
- **window**: 窗口大小和属性设置

## 注意事项

⚠️ **使用声明**: 本项目仅供学习研究使用，请遵守当地法律法规，支持正版图书。

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！

---

**版本**: 0.1.0  
**最后更新**: 2024
