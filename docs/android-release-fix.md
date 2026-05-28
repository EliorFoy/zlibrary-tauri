# Android Release 构建问题复盘

## 结论

Android release APK 构建问题已经解决。验证的 GitHub Actions run：

- https://github.com/EliorFoy/zlibrary-tauri/actions/runs/26578686775
- commit: `f25c3e8 fix: build Android release with Tauri GUI feature`
- 结果：`android` job 成功，APK artifact 上传成功。

## 失败现象

最初失败发生在 Android job 的 `Build Android APK` 步骤。

没有手动创建 Gradle 文件时，Gradle 报：

```text
Could not read script 'src-tauri/gen/android/app/tauri.build.gradle.kts' as it does not exist.
```

后来 workflow 增加了空壳文件：

```yaml
echo "// Tauri Android Settings" > src-tauri/gen/android/tauri.settings.gradle
echo "// Tauri Build Settings" > src-tauri/gen/android/app/tauri.build.gradle.kts
```

这让缺文件错误消失，但随后 Kotlin 编译失败：

```text
Execution failed for task ':app:compileUniversalReleaseKotlin'
TauriActivity.kt: Unresolved reference: app
TauriActivity.kt: Unresolved reference: PluginManager
```

## 根因

`tauri.settings.gradle` 和 `app/tauri.build.gradle.kts` 不是普通占位文件，而是 Tauri 在 Android Rust 构建阶段由 `tauri-build` 自动生成的 Gradle 片段。

这些文件必须包含 Tauri Android core 以及插件 Android project 的依赖，例如 `:tauri-android`。如果它们不存在，Gradle 无法 apply；如果它们是空壳，Gradle 能继续执行，但 Kotlin 源码里的 `app.tauri.plugin.PluginManager` 找不到对应依赖。

更深一层的原因是 Android workflow 调用：

```yaml
cargo tauri android build --apk
```

没有显式启用本项目的 `gui` feature。项目的 `tauri` 与 `tauri-build` 依赖都挂在 `gui` feature 后面：

```toml
gui = ["dep:tauri", "dep:tauri-plugin-dialog", "dep:tauri-plugin-fs", "dep:tauri-plugin-shell", "dep:tauri-build"]
```

Android 构建没有带 `gui` feature 时，`build.rs` 中的 `tauri_build::build()` 不会运行，Tauri 也就不会生成正确的 Android Gradle 文件。

## 修复方式

修复点在 `.github/workflows/release.yml` 的 Android job：

1. 删除 `cargo install tauri-cli --version "^2"`，避免 CI 上 Tauri CLI 版本漂移。
2. 改用 `npm ci` 后安装的本地 Tauri CLI。
3. 删除手动创建空壳 Gradle 文件的步骤。
4. Android build 显式传入 `--features gui --ci`。

修复后的核心步骤：

```yaml
- name: Init Android project
  run: npm run tauri -- android init --skip-targets-install --ci

- name: Build Android APK
  run: npm run tauri -- android build --apk --features gui --ci
```

## 验证

推送 commit `f25c3e8` 后触发 workflow：

- desktop: Windows / macOS / Linux 全部成功
- android: `Build Android APK` 成功
- `Upload APK artifacts` 成功

因此 Android release 构建链路已经恢复。

## 后续注意

- 不要在 workflow 中手动写入空壳 `tauri.settings.gradle` 或 `tauri.build.gradle.kts`。
- Android 构建必须带上项目真正需要的 cargo feature，本项目目前是 `gui`。
- CI 优先使用 package-lock 锁定的 npm Tauri CLI，少用 `cargo install tauri-cli --version "^2"` 这种浮动版本。
