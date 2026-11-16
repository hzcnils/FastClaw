# 参与贡献

1. 从公开主分支创建功能分支。
2. 不提交运行时数据库、下载服务目录、模型、二进制、构建产物、`.env` 或 `config.json`。
3. 提交前运行：

```bash
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
powershell -ExecutionPolicy Bypass -File scripts/check-public-release.ps1
```

涉及数据库结构时新增迁移，不修改已经发布过的迁移；本次首次公开发布前的凭据清理属于例外。Pull Request 应说明功能变化、验证结果和配置兼容性。
