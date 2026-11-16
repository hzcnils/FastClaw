# 第三方源码说明

核对日期：2026-09-30。依据是当时的 `package-lock.json` 与 `src-tauri/Cargo.lock`。

本仓库直接纳入的源码由西安汇智信息科技有限公司开发，随 FastClaw 再分发。各目录自己的许可证字段如下。

| 目录 | 开发方 | 许可证字段 |
| --- | --- | --- |
| `chat-plan-packages/` | 西安汇智信息科技有限公司 | 各 npm 包 `Apache-2.0` |
| `download-packages/` | 西安汇智信息科技有限公司 | npm 包 `Apache-2.0`；`tauri-plugin-shared`、`tauri-plugin-download-center` 为 `MIT OR Apache-2.0` |
| `crates/webview2-checker/` | 西安汇智信息科技有限公司 | `MIT` |
| `src-tauri/vendors/jmisformu/` | 随本仓库分发的 path crate | `MIT OR Apache-2.0`。crate 源码里没有单独的版权人声明 |

`jmisformu` 关闭了默认的 `pdf-ratex` feature，锁文件中没有 RaTeX 的 git 依赖。

## 核对方式

- npm：`package-lock.json` 中 791 个包。本机已安装的包读取了 `package.json` 的 `license` 字段；字段为空时再读包内 `LICENSE` 文件。
- 本机未安装的 98 个可选平台包（其他操作系统或 CPU 的预编译二进制）按其已安装的同族包，或 npm registry 上对应版本的 `license` 字段核对。
- Cargo：`cargo metadata --locked` 返回 927 个包（含 FastClaw 自身与 path crate），使用每个 crate 的 `license` 字段。

同一包给出多个许可证时，本项目选用宽松许可证：`MIT` 或 `Apache-2.0`。不选用 GPL 或 LGPL 选项。

## 需要单独说明的包

| 包 | 声明许可证 | 处理 |
| --- | --- | --- |
| `@tinyflow-ai/ui@1.3.7` | LGPL-3.0-or-later | 写在根目录 `package.json` 的 dependencies 里。当前 `src/` 没有引用它，安装包构建不会把它打进前端产物。`npm install` 仍会下载该包。保留它时，按 LGPL-3.0-or-later 提供对应源码。 |
| `lightningcss@1.32.0`、`lightningcss-win32-x64-msvc@1.32.0` | MPL-2.0 | Tailwind 的构建依赖。未修改这些文件。 |
| `caniuse-lite@1.0.30001810` | CC-BY-4.0 | 构建期浏览器兼容数据。数据来自 caniuse.com。 |
| `dompurify@3.4.15` | MPL-2.0 OR Apache-2.0 | 选用 Apache-2.0。 |
| `jszip@3.10.1` | MIT OR GPL-3.0-or-later | 选用 MIT。 |
| `pizzip@3.2.0` | MIT OR GPL-3.0 | 选用 MIT。 |
| `rgbcolor@1.0.1` | 非 SPDX。包内 `FEEL-FREE.md` 允许使用该代码 | `canvg@3.0.11` 的依赖。 |
| `cssparser` 0.31.2、0.36.0；`cssparser-macros` 0.6.1；`dtoa-short` 0.3.5；`option-ext` 0.2.0；`selectors` 0.25.0、0.36.1；`usvg`、`usvg-parser`、`usvg-text-layout`、`usvg-tree` 0.35.0 | MPL-2.0 | 编进 Rust 二进制。这些文件保持上游版本，源码可按锁文件版本从 crates.io 获取。 |
| `webpki-roots@1.0.9` | CDLA-Permissive-2.0 | 含 Mozilla CA 证书列表。 |
| `icu_collections`、`icu_locale_core`、`icu_normalizer`、`icu_normalizer_data`、`icu_properties`、`icu_properties_data`、`icu_provider`、`litemap`、`potential_utf`、`tinystr`、`writeable`、`yoke`、`yoke-derive`、`zerofrom`、`zerofrom-derive`、`zerotrie`、`zerovec`、`zerovec-derive`（均为 2.2.0 或对应 0.x 锁版本） | Unicode-3.0 | 含 Unicode 字符数据。使用条款见 <https://www.unicode.org/license.txt>。 |
| `unicode-ident@1.0.24` | (MIT OR Apache-2.0) AND Unicode-3.0 | 选用 MIT OR Apache-2.0，并保留 Unicode-3.0 数据声明。 |
| `r-efi` 5.3.0、6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | 选用 MIT OR Apache-2.0。 |

其余已安装 npm 包与其余 Cargo 包的声明许可证是 MIT、Apache-2.0、BSD-2-Clause、BSD-3-Clause、ISC、Zlib、0BSD、Unlicense、BlueOak-1.0.0、Python-2.0、BSL-1.0、CC0-1.0、MIT-0，或这些许可证的 OR / AND 组合。`libfuzzer-sys@0.4.13` 为 (MIT OR Apache-2.0) AND NCSA，`target-lexicon` 为 Apache-2.0 WITH LLVM-exception。

本机未安装、已按同族包或 registry 核对的可选平台包：`esbuild`、`rollup`、`@tailwindcss/oxide`、`@parcel/watcher`、`@napi-rs/lzma`、`fsevents` 为 MIT；`@tauri-apps/cli` 为 Apache-2.0 OR MIT。`lightningcss` 的其他平台二进制与已安装的 Windows 包同为 MPL-2.0。

直接依赖里，除上表已列的 `@tinyflow-ai/ui`、`dompurify`、`jszip` 外，根目录 `package.json` 的 dependencies 与 devDependencies 为 MIT、Apache-2.0、Apache-2.0 OR MIT、BSD-3-Clause（`highlight.js`）或 ISC（`lucide-vue-next`）。
