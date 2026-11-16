# FastClaw

#### 介绍

FastClaw 是桌面 AI 助手，公开版保留智能对话、知识库和模型中台。知识库用于管理资料、文件和切片；模型中台提供模型、工具、Skills、MCP、WSP 和用量看板。

#### 软件架构

FastClaw 采用 Tauri 2、Vue 3 与 Rust 构建桌面应用：

- 前端使用 Vue 3、TypeScript、Pinia、Vue Router 和 Element Plus；
- 后端使用 Rust、Tauri 2、Axum 和 SQLite；
- 前后端通过 Tauri IPC 与本地 HTTP 服务通信；
- FastKG、向量库、Embedding 和重排序等能力通过外部服务地址接入；
- 模型密钥通过系统凭据存储或环境变量管理，不写入仓库配置文件。

仓库不内置 FastKG、Qdrant、llama-server、File2Md、Shimmy、XUpdate、模型文件或其他外部服务可执行文件。知识库服务包的下载地址在设置的网络服务里填写，公开版默认留空，填好后本机没有 FastKG 时才会下载安装。

#### 安装教程

1. 准备 Node.js 22.22+、npm、Rust stable，以及 Tauri 2 对应平台的系统依赖。Windows 还需要 WebView2 与 C++ Build Tools。
2. 克隆仓库并进入项目目录：

   ```bash
   git clone https://gitee.com/hzcnils/fastclaw.git
   cd fastclaw
   ```

3. 安装依赖并构建前端：

   ```bash
   npm install
   npm run build
   ```

4. 启动桌面应用：

   ```bash
   npm run tauri:dev
   ```

仅调试前端时，可运行：

```bash
npm run dev
```

#### 使用说明

应用会在可执行文件同目录创建 `config.json`。源码开发时可参考 [`config.example.json`](config.example.json)，主要配置项包括：

- `remote_servers.download_api_base_url`：FastKG 公网下载地址；
- `knowledge.vector_backend`、`knowledge.vector_url`：Qdrant、Milvus 等向量库配置；
- `knowledge.embedding_url`：OpenAI 兼容的 Embedding 服务地址；
- `knowledge.reranker_url`：重排序服务地址，留空时禁用；
- `services.*`：FastKG、FastPlan 等外部服务地址。

密钥不写入 `config.json`。配置文件只保存环境变量名称，实际密钥通过环境变量传入：

```powershell
$env:FASTCLAW_VECTOR_API_KEY = "..."
$env:FASTCLAW_EMBEDDING_API_KEY = "..."
$env:FASTCLAW_RERANKER_API_KEY = "..."
npm run tauri:dev
```

模型供应商的 API Key 可在应用的模型设置中配置，由系统凭据存储管理。请勿提交 `.env`、`config.json`、数据库文件或运行时目录。

发布前可执行以下命令进行验证：

```bash
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
powershell -ExecutionPolicy Bypass -File scripts/check-public-release.ps1
```
