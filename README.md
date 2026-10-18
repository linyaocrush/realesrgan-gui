# Real-ESRGAN GUI

[![build](https://github.com/linyaocrush/realesrgan-gui/actions/workflows/build.yml/badge.svg)](https://github.com/linyaocrush/realesrgan-gui/actions/workflows/build.yml)

实用、美观的图片放大工具 [Real-ESRGAN](https://github.com/xinntao/Real-ESRGAN) 的图形界面。

本项目是基于 [TransparentLC/realesrgan-gui](https://github.com/TransparentLC/realesrgan-gui) 的 **Rust 重写版**：界面、功能、配置格式都对齐原版（Python + tkinter），但改用 [Rust](https://www.rust-lang.org/) + [egui](https://github.com/emilk/egui) 实现，启动更快、体积更小，也不再需要 Python 运行时。

* 仓库：<https://github.com/linyaocrush/realesrgan-gui>
* 许可证：[GNU AGPL-3.0](LICENSE)（与原项目一致）

## 相对于原版（Python 版）的改动

* 全部代码用 Rust 重写，界面使用 `eframe` / `egui`；配色与控件样式参考原版使用的 Sun Valley 主题，尽量还原原版的观感（含深色模式）。
* **启动速度明显提升**：发布版冷启动到窗口出现通常只需几十毫秒，整个程序是一个约 8.5 MB 的原生可执行文件，不需要随附 Python 运行时。
* Python 版的相关代码（`main.py`、`task.py`、`param.py`、`i18n.py`、`define.py` 以及 PyInstaller/macOS 打包脚本）已全部移除。

## 功能

* 任意尺寸放大
  * Real-ESRGAN 只能以固定倍率（取决于模型）放大，本程序通过多次放大后再用常规缩放算法降采样，实现任意尺寸。
  * 例如把 640x360 的图片用 2x 模型放大到宽度 1600：先放大到 1280x720，再放大到 2560x1440，最后降采样到 1600x900。
* 批量处理文件或整个文件夹
* GIF 处理：拆分出每一帧、记录时长，逐个放大后重新合并（可选的透明色额外处理）
* 支持 [Real-CUGAN](https://github.com/nihui/realcugan-ncnn-vulkan) 模型
* 有损压缩（JPEG / WebP）与自定义压缩 / 后处理命令
* 拖拽文件或文件夹到窗口即可设定输入
* 深色 / 浅色模式跟随系统自动切换
* 14 种界面语言（简体/繁体中文、英语、乌克兰语、土耳其语、西班牙语、法语）

## 使用方法

1. 从 [Real-ESRGAN-ncnn-vulkan 的 Release](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/releases) 下载主程序与模型，或使用维护中的替代分支 [upscayl/upscayl-ncnn](https://github.com/upscayl/upscayl-ncnn) 的 `upscayl-bin[.exe]`（会被优先使用）。
2. 把它们与 `realesrgan-gui[.exe]` 放在同一目录。
3. 运行程序，把图片或文件夹拖进窗口（或点“浏览”选择），设定放大方式后点“开始”。

> [!TIP]
> 没有找到主程序或模型时，界面上会显示红色提示和“下载 Real-ESRGAN”按钮（原版在这种情况下会直接弹窗退出）。

### 从源码构建

需要 Rust 1.89 或以上版本。

```bash
git clone https://github.com/linyaocrush/realesrgan-gui.git
cd realesrgan-gui

# 构建发布版，产物为 rust/target/release/realesrgan-gui[.exe]
cargo build --release --manifest-path rust/Cargo.toml

# 运行单元测试
cargo test --manifest-path rust/Cargo.toml
```

## 目录结构

```text
rust/
├── Cargo.toml
├── build.rs            # 在 Windows 上写入图标、版本信息与应用程序清单
└── src/
    ├── main.rs         # 程序入口
    ├── app.rs          # 应用状态与界面（基本设定 / 高级设定 / 关于）
    ├── theme.rs        # Sun Valley 主题与自绘控件（开关、单选、标签页、数字框等）
    ├── task.rs         # 任务队列：放大、GIF 拆分合并、有损压缩、自定义命令
    ├── config.rs       # config.ini 读写
    ├── i18n.rs         # 翻译（在编译时内嵌 i18n.ini）
    ├── models.rs       # 模型列表（含 Real-CUGAN 的模型组合）
    ├── image_util.rs   # 图像读写与缩放
    ├── win.rs          # 平台集成：主题检测、通知、任务栏进度、标题栏深色模式
    ├── ini.rs          # INI 解析与写出
    └── define.rs       # 路径、外部程序探测、相关链接
i18n.ini                # 界面翻译文本
```

## 配置与翻译

* `config.ini`：与程序放在同一目录，退出时自动保存；删除该文件即可恢复默认设置。
* `i18n.ini`：**在编译时内嵌**到可执行文件中，修改后需要重新构建才能生效。
* 翻译缺失时会回退到英语。

## 与原版的差异 / 已知限制

* 放大本身由外部 ncnn 程序完成，传给它的参数与原版逐项一致，因此**放大结果完全相同**；下述差异只影响 GUI 自己完成的部分。
* 缩放滤波：受 Rust `image` 库的滤波器限制，`Box` → `Triangle`、`Hamming` → `Gaussian` 为近似实现（`Bilinear` → `Triangle`、`Bicubic` → `CatmullRom`、`Lanczos` → `Lanczos3`）。
* 有损 WebP 使用 libwebp（`webp` crate），GIF 使用 `gif` crate，输出文件与原版（Pillow）相比可能在压缩细节、调色板上存在细微差异。
* 目前仅在 Windows 上构建和测试，Linux / macOS 尚未验证。

## 借物表

* [TransparentLC/realesrgan-gui](https://github.com/TransparentLC/realesrgan-gui) 原项目，本项目的界面、功能与配置格式均参考自它
* [Real-ESRGAN](https://github.com/xinntao/Real-ESRGAN) / [Real-ESRGAN-ncnn-vulkan](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan)
* [realcugan-ncnn-vulkan](https://github.com/nihui/realcugan-ncnn-vulkan)
* [Sun-Valley-ttk-theme](https://github.com/rdbende/Sun-Valley-ttk-theme) 配色与控件样式的参考来源
* [egui / eframe](https://github.com/emilk/egui)、[image](https://github.com/image-rs/image)、[gif](https://github.com/image-rs/image-gif)、[webp](https://github.com/jaredforth/webp)、[rfd](https://github.com/PolyMeilex/rfd)、[dark-light](https://github.com/frewsxcv/rust-dark-light)、[notify-rust](https://github.com/hoodie/notify-rust)

## 许可证

[GNU Affero General Public License v3.0](LICENSE)。

本项目是 [TransparentLC/realesrgan-gui](https://github.com/TransparentLC/realesrgan-gui) 的衍生作品，遵循与原项目相同的许可证。