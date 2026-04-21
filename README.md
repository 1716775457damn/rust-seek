<div align="center">

# 🔍 rust-seek

**高性能本地文件搜索工具 — 文件名 + 文本内容，原生 GUI**

[![Release](https://img.shields.io/github/v/release/1716775457damn/rust-seek?style=flat-square&color=e05d2a)](https://github.com/1716775457damn/rust-seek/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/1716775457damn/rust-seek/release.yml?style=flat-square&label=CI)](https://github.com/1716775457damn/rust-seek/actions)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange?style=flat-square)](https://www.rust-lang.org)

单一二进制，无需安装，跨平台运行。搜索百万文件只需数秒，支持正则、GBK 编码、上下文行、深色/浅色主题。

[下载](#-下载) · [功能](#-功能) · [使用方式](#-使用方式) · [架构](#-架构) · [本地构建](#-本地构建)

</div>

---

## 📦 下载

前往 [Releases](https://github.com/1716775457damn/rust-seek/releases) 下载最新版本：

| 平台 | 文件 | 说明 |
|------|------|------|
| Windows | `rust-seek-windows-x86_64.exe` | 双击即用，无需安装 |
| macOS (Apple Silicon) | `rust-seek-macos-aarch64.tar.gz` | M 系列芯片 |
| macOS (Intel) | `rust-seek-macos-x86_64.tar.gz` | x86_64 |
| Linux | `rust-seek-linux-x86_64.tar.gz` | x86_64 |

> **macOS 首次打开提示"未验证的开发者"**：系统设置 → 隐私与安全性 → 仍然打开

---

## ✨ 功能

### 两种搜索模式

| 模式 | 说明 |
|------|------|
| 🗂 文件名 | 按文件/文件夹名搜索，支持正则，显示文件大小和类型图标 |
| 📄 文本 | 搜索文件内容，显示匹配行 + 上下文，支持 UTF-8 / GBK 双编码 |

### 搜索引擎

- **并行遍历**：基于 `ignore` crate（ripgrep 同款），多核并行，速度与 ripgrep 相当
- **内存映射 I/O**：大文件通过 `memmap2` 零拷贝读取，不占用额外内存
- **正则支持**：Rust `regex` crate，DFA 引擎，Unicode 感知
- **纯文本模式**（`F`）：自动 escape 特殊字符，无需手写转义
- **大小写不敏感**（`Aa`）：默认开启，随时切换
- **实时正则校验**：输入时即时检测语法错误，搜索前就能发现问题
- **二进制文件跳过**：自动检测并跳过二进制文件
- **GBK / GB2312 支持**：UTF-8 解码失败时自动回退 GBK，中文文件无乱码
- **结果上限 2000 条**：超出时自动停止后台线程，提示截断

### 结果展示

- **实时流式输出**：结果边搜索边显示，无需等待完成
- **全部高亮**：每行所有匹配位置均以黄色背景标出
- **上下文行**：文本模式显示匹配行的前后各一行，行号绿色标注
- **折叠/展开**：单个文件或全部一键折叠，超过 5 处匹配时按需展开
- **文件类型图标**：⚙ 可执行、📝 代码、🖼 图片、🎬 视频、📦 压缩包等
- **结果过滤**：搜索完成后在结果中二次过滤，无需重新搜索
- **一键复制全部路径**：所有结果路径复制到剪贴板
- **右键菜单**：打开文件、在资源管理器/Finder 中显示、复制路径/文件夹路径

### 界面

- **深色/浅色主题**：点击 ☀️/🌙 切换，或按 `T` 键
- **拖拽设置路径**：将文件夹拖到窗口即可设置搜索目录
- **搜索历史**：路径和关键词历史跨重启保存，点击或 ↑↓ 键快速复用
- **回车搜索**：无需鼠标，输入后直接回车
- **Esc 取消**：随时中断正在运行的搜索
- **窗口标题**：实时显示搜索状态和结果数
- **自动聚焦**：启动即可直接输入，无需点击

---

## 🚀 使用方式

1. 打开 rust-seek
2. 在「路径」框输入或拖入要搜索的目录（默认为当前目录）
3. 选择模式：🗂 文件名 或 📄 文本
4. 输入关键词，按 **Enter** 或点击 **🔍 搜索**
5. 结果实时出现，右键任意结果可打开文件或在资源管理器中定位

```
┌──────────────────────────────────────────────────────────────────────┐
│ 路径: [C:/project  ] 📁  🗂文件名  📄文本  [fn main    ] 🔍搜索  Aa F ☀️│
├──────────────────────────────────────────────────────────────────────┤
│ ▼全部展开  ▶全部折叠  📋复制全部路径          过滤: [      ] ✕      │
├──────────────────────────────────────────────────────────────────────┤
│ ▼ 📝 src/main.rs  (1 处匹配)                                         │
│    0   mod app;                                                      │
│    1:  fn main() -> eframe::Result {                                 │
│    2   let options = eframe::NativeOptions {                         │
│                                                                      │
│ ▼ 📝 src/app.rs  (3 处匹配)                                          │
│   40   // start search                                               │
│   41:  fn start_search(&mut self) {                                  │
│   42   if self.pattern.is_empty() { return; }                        │
├──────────────────────────────────────────────────────────────────────┤
│ 4 处匹配，共 2 个文件 (8ms)                                           │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 🏗 架构

```
src/
├── main.rs      # 入口，窗口配置，内嵌 CJK 字体（NotoSansSC）
├── app.rs       # GUI：工具栏、结果列表、历史、拖拽、右键菜单
├── searcher.rs  # 核心引擎：mmap 读取、正则匹配、GBK 解码、二进制检测
└── theme.rs     # 深色/浅色主题 visuals
```

| 组件 | Crate | 说明 |
|------|-------|------|
| GUI | `egui` / `eframe` | 即时模式，原生渲染，无 Electron |
| 目录遍历 | `ignore` | ripgrep 同款，并行 + gitignore 感知 |
| 文件读取 | `memmap2` | 内存映射，大文件零拷贝 |
| 正则匹配 | `regex` | DFA 引擎，Unicode 感知 |
| 编码检测 | `encoding_rs` | UTF-8 + GBK/GB2312 回退 |
| 文件对话框 | `rfd` | 原生文件选择器 |
| 持久化 | `serde_json` | 搜索历史和偏好设置 |

---

## ⚡ 性能

- **大小写不敏感搜索 10 万文件**：< 2 秒（8 核机器）
- **内存映射**：10 MB 以内文件零拷贝，不全量加载进内存
- **多核线性扩展**：线程数 = CPU 核心数
- **`Arc` 共享上下文行**：每个匹配零额外堆分配
- **正则缓存**：每次按键不重新编译，仅在 pattern/选项变化时重建
- **结果一次性排序**：搜索完成后 O(n log n)，不在插入时排序
- **过滤索引缓存**：过滤结果缓存，仅在结果或过滤词变化时重建

---

## 🔧 本地构建

需要 [Rust](https://rustup.rs/) stable 工具链。

```bash
git clone https://github.com/1716775457damn/rust-seek.git
cd rust-seek
cargo build --release

# 产物
./target/release/rust-seek        # macOS / Linux
./target/release/rust-seek.exe    # Windows
```

Linux 额外依赖：

```bash
sudo apt-get install -y \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libssl-dev libgtk-3-dev
```

---

## 🤖 CI/CD

推送 `v*` tag 触发四平台并行构建：

```bash
git tag v4.5.0
git push origin v4.5.0
```

GitHub Actions 在 Windows / macOS ARM / macOS Intel / Ubuntu 上并行构建，约 5 分钟后在 [Releases](https://github.com/1716775457damn/rust-seek/releases) 生成全部二进制。

---

## 📄 License

MIT © 2025
