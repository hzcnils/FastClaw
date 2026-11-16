// FastClaw · clips 仓库层
use sqlx::SqlitePool;

pub async fn list(
    _pool: &SqlitePool,
    _filter: Option<&str>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    // TODO: 实现具体查询
    Ok(vec![])
}

pub async fn insert(_pool: &SqlitePool, _id: &str, _data: serde_json::Value) -> anyhow::Result<()> {
    // TODO: 实现插入
    Ok(())
}

pub async fn update(_pool: &SqlitePool, _id: &str, _data: serde_json::Value) -> anyhow::Result<()> {
    // TODO: 实现更新
    Ok(())
}

pub async fn delete(_pool: &SqlitePool, _id: &str) -> anyhow::Result<()> {
    // TODO: 实现删除
    Ok(())
}

pub async fn toggle(_pool: &SqlitePool, _id: &str, _enabled: bool) -> anyhow::Result<()> {
    // TODO: 实现启用/禁用
    Ok(())
}
