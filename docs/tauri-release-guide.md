# Tauri 发布流程备忘

这份文档只作为本地发布备忘，不需要提交到仓库。

## 当前结论

本项目现在的发布方式是：

- 普通 push 到 `master` / `main`：只跑 CI 和跨平台构建，并上传 Actions artifacts。
- 推送 `v*` tag：创建或更新 GitHub Release，并把 GUI、CLI、Android APK 上传到 Release 页面。
- macOS 桌面版会在 `macos-latest` job 中构建。
- iOS 暂时没有发布 job。当前项目和本地 Tauri CLI 没有可用的 `tauri ios` 子命令；真正发布 iOS 还需要 Apple 证书、provisioning profile、签名和对应的 Tauri iOS 初始化流程。

## 每次发布前必须做的事

1. 确认版本号统一。

需要保持以下文件版本一致：

```text
package.json
package-lock.json
src-tauri/Cargo.toml
src-tauri/tauri.conf.json
```

例如发布 `v0.1.3`，这些文件里的版本应为 `0.1.3`。

2. 本地跑基础检查。

```powershell
npm run build
cargo check --manifest-path src-tauri\Cargo.toml --features gui
cargo check --manifest-path src-tauri\Cargo.toml --features cli
```

有条件的话再跑一次本机桌面打包：

```powershell
npm run tauri build
```

3. 提交并推送代码。

注意不要把本地复盘文档提交进去：

```powershell
git status --short
git add .github/workflows/release.yml package.json package-lock.json src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/src src
git commit -m "chore: prepare release v0.1.3"
git push origin master
```

如果只是代码修复，commit message 按实际内容写，不要为了发布乱写。

4. 等普通 push 的 CI 和 Build and Release 先过。

普通 push 不会发 Release，但能验证：

- Windows GUI artifact
- macOS GUI artifact
- Linux GUI artifact
- CLI release build
- Android artifact job，必要时可先忽略 Android

查看命令：

```powershell
gh run list --limit 10
gh run watch <run-id> --exit-status
```

## 正式发布

确认普通 push 的关键 job 通过后，再打 tag：

```powershell
git tag v0.1.3
git push origin v0.1.3
```

推送 tag 后，`.github/workflows/release.yml` 会自动：

1. 创建或更新 `Z-Library NoProxy v0.1.3` Release。
2. 生成发布说明。
3. 构建并上传桌面 GUI 安装包。
4. 构建并上传 CLI 压缩包。
5. 构建并上传 Android APK。

如果 tag 已经存在但要重跑同一个版本，不要随便改远程 tag。优先修 workflow 后删除失败 Release 资产重新上传，或者发下一个 patch 版本。

## Release 页面应出现的资产

理想情况下 Release 页面至少应有：

```text
Z-Library-NoProxy-GUI-Windows-x64-MSI.msi
Z-Library-NoProxy-GUI-Windows-x64-Setup.exe
Z-Library-NoProxy-GUI-macOS-*-DMG.dmg
Z-Library-NoProxy-GUI-Linux-x64-AppImage.AppImage
Z-Library-NoProxy-GUI-Linux-x64-DEB.deb
Z-Library-NoProxy-GUI-Linux-x64-RPM.rpm
Z-Library-NoProxy-CLI-Windows-x64.zip
Z-Library-NoProxy-CLI-macOS-*.tar.gz
Z-Library-NoProxy-CLI-Linux-x64.tar.gz
```

Android APK 名称以 `Z-Library-NoProxy-Android-*` 开头；如果本次先不管 Android，就至少确认桌面 GUI 和 CLI 都已经上传。

## 常见坑

- 只 push commit 不会发 Release，只会生成 artifacts。
- 版本号不统一会导致 Release 标题、tag、安装包版本互相不一致。
- 不要手动创建 Tauri Android Gradle 占位文件；Android 构建必须让 Tauri 自己生成。
- Android 构建必须带 `--features gui --ci`。
- Linux AppImage 不能把数据库、日志、缓存写到可执行文件目录；AppImage 运行时目录可能是 `/tmp/.mount_*`。
- iOS 不是“顺手加一个 job”就能发，需要完整签名链路。

## 发布后检查

```powershell
gh release view v0.1.3 --web
gh release view v0.1.3 --json name,tagName,isDraft,isPrerelease,assets
```

检查点：

- Release 不是 draft。
- Release title 和 tag 一致。
- GUI / CLI 文件都在 Release assets 里。
- macOS 产物存在。
- Linux 至少有 AppImage，最好也有 deb/rpm。
- Windows 有 msi 和 setup exe。

