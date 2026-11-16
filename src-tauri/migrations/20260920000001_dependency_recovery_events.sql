-- 五个内置依赖服务不属于 wsp_services，不能写入带该外键的 wsp_event_logs。
CREATE TABLE IF NOT EXISTS dependency_recovery_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id TEXT NOT NULL,
    generation INTEGER NOT NULL,
    phase TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    detail TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_dependency_recovery_events_service
    ON dependency_recovery_events(service_id, id DESC);
