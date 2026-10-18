# 贡献这个项目

感谢你对 Real-ESRGAN GUI 这个项目的贡献～ 如果你觉得它帮到了你，也欢迎为这个项目点一个 ⭐Star。

## 报告 Bug，请求新功能

请先查找 [已有的 Issue](https://github.com/linyaocrush/realesrgan-gui/issues?q=is%3Aissue)，如果仍然有疑问，再[提交新的 Issue](https://github.com/linyaocrush/realesrgan-gui/issues/new)。

如果遇到了 bug，请提供与 bug 有关的重要信息，例如运行环境、日志输出（程序目录下的 `output.log`）、复现过程、会触发 bug 的图片等。

> [!TIP]
> 请先确认 bug 确实与这个 GUI 相关，而不是来自 Real-ESRGAN 本身。
>
> 放大功能由外部的 `realesrgan-ncnn-vulkan` 等程序完成，你可以在命令行中自行执行程序输出的那条命令，以确认问题是否出在 Real-ESRGAN 上。

## 提交代码

* 需要 Rust 1.89 或以上版本。
* 提交前请确认 `cargo test --manifest-path rust/Cargo.toml` 全部通过，并且 `cargo build --release --manifest-path rust/Cargo.toml` 没有警告。
* 请尽量保持与现有模块一致的代码风格和结构。
* 你也可以先自行修复 bug 或实现新功能并提交 Pull Request，在 merge 之前我可能会提出 review。

## 添加或改进翻译

界面文本全部放在 [`i18n.ini`](i18n.ini) 中，一个 section 可以对应多个语言（用逗号分隔）：

```ini
[en_US, en_GB]
Input = Input (file or folder)
ToastCompletedMessage = The files have been saved to: {0}
                        Elapsed time: {1:.03f}s
```

* `{0}`、`{1:.03f}` 这样的部分是格式化占位符，**需要保留**。
* 多行文本使用缩进续行，会在读取时用换行符连接。
* 某个语言缺少翻译时会回退到英语。

添加新语言时还需要在 [`rust/src/i18n.rs`](rust/src/i18n.rs) 的 `LOCALE_NAMES` 中登记语言的标识和显示名称（即语言下拉框中显示的文字）。

> [!IMPORTANT]
> `i18n.ini` 是在**编译时内嵌**到可执行文件中的（`include_str!`），修改后需要重新构建才能生效。