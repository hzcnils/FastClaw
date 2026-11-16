# 公开发布说明

## 当前发布结构

- FastKG 及其 Qdrant、llama-server、模型文件不进入 Git。知识库下载地址在设置的网络服务里填写，公开版默认留空；填好后，本机没有 FastKG 时才会下载并安装。
- Tauri 配置不再声明 `externalBin` 或 `resources`，源码构建不依赖仓库中的 sidecar 二进制。
- 原 Git submodule 与仓库外 Rust path dependency 已转为仓库内源码目录；下载中心源码位于 `download-packages/`。
- `public/WebResource`、账户登录/支付模块和 Mnemosyne 记忆服务已从公开版源码及运行入口移除。
- 左侧导航仅保留“智能对话”“知识管理”和“模型中台”；模型中台继续提供模型管理、全部工具、Skills、MCP、WSP 管理和调用看板子菜单。
- 数据库快照、模型清单导出、性能 trace、服务二进制和压缩构建物不发布。
- 外部向量库、Embedding、Reranker 地址位于 `config.json` 的 `knowledge` 节；密钥只从环境变量读取。

项目许可证为 Apache License 2.0，文本在根目录 `LICENSE`。FastClaw 名称、Logo、界面图片和示例数据均为西安汇智信息科技有限公司所有，随本仓库按该许可证再分发。仓库不包含单独的字体文件。

## 发布前必须确认

1. `@tinyflow-ai/ui@1.3.7` 的许可证是 LGPL-3.0-or-later，当前源码没有引用它。发布前决定从 `package.json` 移除，或保留并在分发时按 LGPL 提供该包源码。其余 npm 与 Cargo 依赖已按锁文件核对，结果在 `THIRD_PARTY_NOTICES.md`。
2. 对曾经出现过的真实 API Key 执行服务端吊销/轮换。仅从当前文件删除并不能使旧密钥重新安全。当前这 7 次提交中未发现可用的真实密钥。
3. 只发布清洁的单根提交历史；不要把原仓库含密钥、数据库与大模型的祖先提交推送到公开远端。推送到 Gitee 时不保留私人作者邮箱。
4. 在全新 Windows 环境完成 `npm install`、前端构建、Rust 检查；填写知识库下载地址后再做首次 FastKG 下载和知识库健康检查。

## 发布边界

知识库服务包不是此源码仓库的一部分。公开版不预置下载地址；填写地址后客户端才会下载，并在安装前校验接口返回的 SHA-256。服务包的可用性、签名、许可证与再分发条款需要独立确认。
