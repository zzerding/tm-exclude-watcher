<!-- ABOUTME: 记录 tm-watcher 的 GitHub Release、Homebrew tap 和真实 macOS 发布验收流程。 -->

# 发布流程

本文档记录 `tm-watcher` 的长期发布决策。发布实现、CI workflow 和 Homebrew formula 应以本文档为准。

## 发布身份

- 命令名和 crate 名：`tm-watcher`
- 源码仓库：`zzerding/tm-exclude-watcher`
- Homebrew tap：`zzerding/homebrew-tap`
- Homebrew formula：`Formula/tm-watcher.rb`
- 用户安装命令：

```bash
brew tap zzerding/tap
brew install tm-watcher
```

## 版本策略

- 发布由 `v*` git tag 驱动。
- tag 版本必须与 `Cargo.toml` 的 `package.version` 一致。
- 版本不一致时，release workflow 必须失败，不自动修改版本或提交源码。
- `--version` 输出只来自 `env!("CARGO_PKG_VERSION")`。
- RC 只发布 GitHub prerelease，不更新 Homebrew formula。
- stable 发布 GitHub Release，并自动更新 Homebrew formula。
- RC 允许修复发布验证发现的问题，但不加入新功能。

示例：

```text
v0.2.0-rc.2 -> GitHub prerelease + 二进制资产，不更新 Homebrew tap
v0.2.0      -> GitHub stable release + 二进制资产，更新 Homebrew tap
```

RC release 不标记为 latest；只有 stable release 才是 latest。

## 发布产物

只发布 macOS 双架构二进制：

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

不发布 Linux 或 Windows 产物。

每个架构独立 tarball，不做 universal binary。tarball 命名带版本号：

```text
tm-watcher-v0.2.0-aarch64-apple-darwin.tar.gz
tm-watcher-v0.2.0-x86_64-apple-darwin.tar.gz
```

tarball 内容保持最小：

```text
tm-watcher
README.md
LICENSE
```

Release assets 只上传 tarball 和聚合 checksum 文件：

```text
SHA256SUMS
```

不上传裸二进制。

## CI 和打包

- 使用 `cargo-dist` 负责 macOS 双架构构建、打包、checksum 和 GitHub Release assets。
- 发布 workflow 只由 `v*` tag 创建正式 release。
- `workflow_dispatch` 可用于验证构建链路，但不创建正式 release。
- stable 发布时额外执行 Homebrew tap 更新步骤。
- Cargo 构建缓存使用成熟 action，例如 `Swatinem/rust-cache`，不要手写复杂 cache key。

当前 GitHub Release 资产切片先接入 macOS tarball 发布链路，不更新 Homebrew tap。
workflow 使用 `cargo-dist` 生成 tarball 和 checksum，再按本项目资产策略将文件名规范化为带版本号的 tarball 和聚合 `SHA256SUMS`。
Homebrew tap 更新由后续 stable 发布切片接入。

发布门禁：

```bash
rtk cargo fmt --check
rtk cargo clippy --all-targets -- -D warnings
rtk cargo test
rtk cargo run -- --version
rtk cargo run -- --help
```

GitHub Actions 不执行真实 `tmutil` 或 LaunchAgent E2E。

### macOS runner

当前发布链路使用：

```text
x86_64-apple-darwin  -> macos-15-intel
aarch64-apple-darwin -> macos-15
```

使用 `macos-15` 系列 runner，避免依赖已进入迁移窗口的 `macos-14` 标签。

## Homebrew tap

stable 发布自动更新 `zzerding/homebrew-tap`。更新失败时，stable release workflow 必须失败。

跨仓库写权限使用 fine-grained GitHub PAT：

- secret 名：`HOMEBREW_TAP_TOKEN`
- 权限范围：只允许 `zzerding/homebrew-tap` 的 Contents read/write

不要使用全权限 classic PAT。

formula 下载 GitHub Release 预编译二进制，不从源码编译，不依赖 Rust toolchain：

```ruby
desc "Automatically manage macOS Time Machine exclusions for development directories"
homepage "https://github.com/zzerding/tm-exclude-watcher"
license "MIT"

if Hardware::CPU.arm?
  url "https://github.com/zzerding/tm-exclude-watcher/releases/download/v0.2.0/tm-watcher-v0.2.0-aarch64-apple-darwin.tar.gz"
  sha256 "..."
else
  url "https://github.com/zzerding/tm-exclude-watcher/releases/download/v0.2.0/tm-watcher-v0.2.0-x86_64-apple-darwin.tar.gz"
  sha256 "..."
end

def install
  bin.install "tm-watcher"
end

test do
  assert_match version.to_s, shell_output("#{bin}/tm-watcher --version")
  assert_match "tm-watcher", shell_output("#{bin}/tm-watcher --help")
end
```

formula 不定义 `service do`。Homebrew 只负责安装二进制，daemon 生命周期由 `tm-watcher start` / `tm-watcher stop` / `tm-watcher status` 管理。

formula caveats：

```text
tm-watcher is installed but not started automatically.

To enable background monitoring:
  tm-watcher start

To check daemon status:
  tm-watcher status

To stop background monitoring:
  tm-watcher stop
```

tap 自动更新提交信息：

```text
Update tm-watcher to 0.2.0
```

如果 stable tag 已创建但 tap 更新失败，不删除 tag、不重发同名 release。修复 workflow 或 tap 权限后，重新运行同一个 release workflow。workflow 应保持幂等：资产和 formula 可重复生成，formula 内容无变化时跳过提交并成功退出。

## 发布前产品范围

v0.2.0 发布准备必须包含：

- `--help` / `-h`
- `--version` / `-V`
- 根目录 `LICENSE`
- `Cargo.toml` 发布元数据
- `cargo-dist` release workflow
- stable 发布自动更新 Homebrew tap
- `status` 识别 LaunchAgent plist 指向旧二进制路径，并提示 `tm-watcher stop && tm-watcher start`
- 本文档

v0.2.0 不加入：

- `restart`
- `doctor`
- `logs`
- `config`
- glob 规则
- GUI 状态栏应用
- code signing 或 notarization
- Homebrew `service do`

## Release notes

Release notes 先使用 GitHub 自动生成。

RC release 需标注：

```text
This prerelease is for validating macOS daemon behavior and the release pipeline. It is not recommended for long-term regular use.
```

stable release 需补充 Homebrew 安装方式：

```bash
brew tap zzerding/tap
brew install tm-watcher
```

## 本地 E2E 验收

真实 macOS / Time Machine 行为不放进 CI，stable 发布前必须在本地真机验证。

Required：

- Apple Silicon 真机 E2E
- GitHub Actions Intel artifact 原生构建和 `--help` / `--version` 冒烟

Optional：

- Intel 真机 E2E

### 基础命令

```bash
tm-watcher --version
tm-watcher --help
tm-watcher start
tm-watcher status
tm-watcher stop
```

### 手动扫描和 tmutil 真值验证

```bash
rm -rf /tmp/tm-watcher-e2e
mkdir -p /tmp/tm-watcher-e2e/project/node_modules
tm-watcher scan /tmp/tm-watcher-e2e
tmutil isexcluded /tmp/tm-watcher-e2e/project/node_modules
rm -rf /tmp/tm-watcher-e2e
tm-watcher clean
```

`tmutil isexcluded` 必须显示测试目录已被 Time Machine 排除。

### 实时监控

优先用前台 `watch` 验证核心实时监控，避免修改真实全局配置。

```bash
rm -rf /tmp/tm-watcher-e2e
mkdir -p /tmp/tm-watcher-e2e/project
tm-watcher watch /tmp/tm-watcher-e2e
```

在另一个终端创建目录：

```bash
mkdir -p /tmp/tm-watcher-e2e/project/node_modules
tmutil isexcluded /tmp/tm-watcher-e2e/project/node_modules
```

等待 `confirmation_delay_seconds` 加少量缓冲后再检查。

### LaunchAgent 生命周期

启动并确认 launchd job：

```bash
tm-watcher start
launchctl print gui/$(id -u)/com.zzerding.tm-watcher
tm-watcher status
tm-watcher stop
```

登录自启不作为 v0.2.0 stable 硬门槛；可选人工验证退出登录后再次执行 `tm-watcher status`。

### 异常退出重启

```bash
tm-watcher start
tm-watcher status
kill -9 <PID>
sleep 3
tm-watcher status
tm-watcher stop
```

期望 `status` 显示新的 PID 或 daemon 仍在运行。

### 正常停止不重启

```bash
tm-watcher start
tm-watcher stop
sleep 3
tm-watcher status
```

期望 daemon 不在运行，plist 已删除或 launchd job 已卸载。

### daemon 日志

```bash
tm-watcher start
ls -l ~/.local/share/tm-watcher/daemon.log
tail -n 50 ~/.local/share/tm-watcher/daemon.log
tm-watcher stop
```

日志中至少应出现：

```text
守护进程启动中
加载配置文件
打开数据库
```

### 默认配置和路径隔离

配置、数据库和日志路径测试使用临时 HOME：

```bash
rm -rf /tmp/tm-watcher-home
HOME=/tmp/tm-watcher-home tm-watcher status
cat /tmp/tm-watcher-home/.config/tm-watcher/config.toml
```

期望生成：

```text
/tmp/tm-watcher-home/.config/tm-watcher/config.toml
/tmp/tm-watcher-home/.local/share/tm-watcher/exclusions.db
```

LaunchAgent / daemon 生命周期测试使用真实 HOME，不使用临时 HOME。

### Homebrew install 和 upgrade

stable 发布后验证：

```bash
brew tap zzerding/tap
brew install tm-watcher
tm-watcher --version
tm-watcher --help
tm-watcher start
tm-watcher status
tm-watcher stop
```

upgrade 场景需验证 `status` 能识别 LaunchAgent plist 指向旧二进制路径，并提示：

```bash
tm-watcher stop && tm-watcher start
```

Homebrew 不自动启动 daemon，不自动重启 daemon。

### 清理测试残留

```bash
tm-watcher stop || true
rm -rf /tmp/tm-watcher-e2e
rm -rf /tmp/tm-watcher-home
tm-watcher clean
```

如果测试期间改过真实配置，发布结束前必须恢复原配置。
