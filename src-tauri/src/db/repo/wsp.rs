// FastClaw · WSP 服务数据仓库
// 对 wsp_services / wsp_runtime_status / wsp_event_logs / wsp_endpoints / wsp_call_logs 五张表的操作

use crate::core::wsp::types::*;
use sqlx::SqlitePool;

/// 获取所有 WSP 服务（含运行状态）
pub async fn list_services_with_status(
    pool: &SqlitePool,
) -> anyhow::Result<Vec<WspServiceWithStatus>> {
    let mut rows = sqlx::query_as::<_, WspServiceWithStatus>(
        r#"
        SELECT s.id, s.name, s.display_name, s.description, s.version, s.category,
               s.categoryName as category_name, s.subCategoryName as sub_category_name,
               s.exe_path, s.port, s.protocol, s.auto_start, s.is_enabled, s.host, s.base_path,
               CAST(r.pid AS INTEGER) as pid,
               r.status as runtime_status,
               r.cpu_percent,
               r.memory_mb,
               r.gpu_memory_mb,
               r.request_count,
               r.error_count,
               r.avg_latency_ms,
               r.restart_count,
               r.started_at as runtime_started_at,
               r.error_message as runtime_error
        FROM wsp_services s
        LEFT JOIN wsp_runtime_status r ON s.id = r.service_id
        WHERE s.is_enabled = 1
        ORDER BY s.sort_order, s.category, s.name
        "#,
    )
    .fetch_all(pool)
    .await?;

    for svc in &mut rows {
        let host = if svc.host.starts_with("http") {
            svc.host.trim_end_matches('/').to_string()
        } else {
            format!("http://{}:{}", svc.host, svc.port)
        };
        let base = svc.base_path.trim_end_matches('/');
        svc.computed_url = format!("{}{}", host, base);
    }

    Ok(rows)
}

/// 获取单个 WSP 服务详情
pub async fn get_service(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<WspService>> {
    let row = sqlx::query_as::<_, WspService>(
        "SELECT id, name, display_name, description, version, category, \
         COALESCE(categoryName, NULL) as category_name, COALESCE(subCategoryName, NULL) as sub_category_name, \
         exe_path, work_dir, args, env_vars, protocol, host, port, base_path, ws_path, \
         health_endpoint, cpu_limit, memory_limit_mb, gpu_required, gpu_device_id, \
         auto_start, start_delay_ms, start_priority, restart_policy, max_restarts, \
         restart_cooldown_ms, health_check_interval_ms, health_check_timeout_ms, \
         startup_timeout_ms, depends_on, is_enabled, is_installed, sort_order, extra_config, created_at, updated_at \
         FROM wsp_services WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按名称查找 WSP 服务  wsp_services 服务内容保存到这个表里
pub async fn get_service_by_name(
    pool: &SqlitePool,
    name: &str,
) -> anyhow::Result<Option<WspService>> {
    let row = sqlx::query_as::<_, WspService>(
        "SELECT id, name, display_name, description, version, category, \
         categoryName as category_name, subCategoryName as sub_category_name, \
         exe_path, work_dir, args, env_vars, protocol, host, port, base_path, ws_path, \
         health_endpoint, cpu_limit, memory_limit_mb, gpu_required, gpu_device_id, \
         auto_start, start_delay_ms, start_priority, restart_policy, max_restarts, \
         restart_cooldown_ms, health_check_interval_ms, health_check_timeout_ms, \
         startup_timeout_ms, depends_on, is_enabled, is_installed, sort_order, extra_config, created_at, updated_at \
         FROM wsp_services WHERE name = ?"
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 注册新 WSP 服务
pub async fn insert_service(pool: &SqlitePool, service: &WspService) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO wsp_services (id, name, display_name, description, version, category, categoryName, subCategoryName,
            exe_path, work_dir, args, env_vars, protocol, host, port, base_path, ws_path,
            health_endpoint, cpu_limit, memory_limit_mb, gpu_required, gpu_device_id,
            auto_start, start_delay_ms, start_priority, restart_policy, max_restarts,
            restart_cooldown_ms, health_check_interval_ms, health_check_timeout_ms,
            startup_timeout_ms, depends_on, is_enabled, is_installed, sort_order, extra_config)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&service.id).bind(&service.name).bind(&service.display_name)
    .bind(&service.description).bind(&service.version).bind(&service.category)
    .bind(&service.category_name).bind(&service.sub_category_name)
    .bind(&service.exe_path).bind(&service.work_dir).bind(&service.args)
    .bind(&service.env_vars).bind(&service.protocol).bind(&service.host)
    .bind(service.port).bind(&service.base_path).bind(&service.ws_path)
    .bind(&service.health_endpoint).bind(service.cpu_limit).bind(service.memory_limit_mb)
    .bind(service.gpu_required).bind(service.gpu_device_id).bind(service.auto_start)
    .bind(service.start_delay_ms).bind(service.start_priority).bind(&service.restart_policy)
    .bind(service.max_restarts).bind(service.restart_cooldown_ms)
    .bind(service.health_check_interval_ms).bind(service.health_check_timeout_ms)
    .bind(service.startup_timeout_ms).bind(&service.depends_on)
    .bind(service.is_enabled).bind(service.is_installed)
    .bind(service.sort_order).bind(&service.extra_config)
    .execute(pool)
    .await?;

    // 初始化运行状态记录（确保所有必填字段都有值）
    sqlx::query(
        r#"
        INSERT INTO wsp_runtime_status (service_id, status, request_count, error_count, restart_count, last_health_ok, updated_at)
        VALUES (?, 'stopped', 0, 0, 0, 0, datetime('now'))
        ON CONFLICT(service_id) DO NOTHING
        "#,
    )
    .bind(&service.id)
    .execute(pool)
    .await?;

    Ok(())
}

/// 更新 WSP 服务
pub async fn update_service(pool: &SqlitePool, service: &WspService) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE wsp_services SET
            name = ?,
            display_name = ?,
            description = ?,
            version = ?,
            category = ?,
            exe_path = ?,
            work_dir = ?,
            args = ?,
            env_vars = ?,
            protocol = ?,
            host = ?,
            port = ?,
            base_path = ?,
            ws_path = ?,
            health_endpoint = ?,
            cpu_limit = ?,
            memory_limit_mb = ?,
            gpu_required = ?,
            gpu_device_id = ?,
            auto_start = ?,
            start_delay_ms = ?,
            start_priority = ?,
            restart_policy = ?,
            max_restarts = ?,
            restart_cooldown_ms = ?,
            health_check_interval_ms = ?,
            health_check_timeout_ms = ?,
            startup_timeout_ms = ?,
            depends_on = ?,
            is_enabled = ?,
            is_installed = ?,
            sort_order = ?,
            extra_config = ?,
            updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(&service.name)
    .bind(&service.display_name)
    .bind(&service.description)
    .bind(&service.version)
    .bind(&service.category)
    .bind(&service.exe_path)
    .bind(&service.work_dir)
    .bind(&service.args)
    .bind(&service.env_vars)
    .bind(&service.protocol)
    .bind(&service.host)
    .bind(service.port)
    .bind(&service.base_path)
    .bind(&service.ws_path)
    .bind(&service.health_endpoint)
    .bind(service.cpu_limit)
    .bind(service.memory_limit_mb)
    .bind(service.gpu_required)
    .bind(service.gpu_device_id)
    .bind(service.auto_start)
    .bind(service.start_delay_ms)
    .bind(service.start_priority)
    .bind(&service.restart_policy)
    .bind(service.max_restarts)
    .bind(service.restart_cooldown_ms)
    .bind(service.health_check_interval_ms)
    .bind(service.health_check_timeout_ms)
    .bind(service.startup_timeout_ms)
    .bind(&service.depends_on)
    .bind(service.is_enabled)
    .bind(service.is_installed)
    .bind(service.sort_order)
    .bind(&service.extra_config)
    .bind(&service.id)
    .execute(pool)
    .await?;

    Ok(())
}
/// 更新运行状态
pub async fn update_runtime_status(
    pool: &SqlitePool,
    service_id: &str,
    status: &WspRuntimeUpdate,
) -> anyhow::Result<()> {
    let final_status = if status.status.is_empty() {
        "stopped"
    } else {
        &status.status
    };
    sqlx::query(
        r#"
        INSERT INTO wsp_runtime_status (service_id, pid, status, cpu_percent, memory_mb,
            gpu_memory_mb, request_count, error_count, restart_count, last_health_ok,
            started_at, stopped_at, error_message, updated_at)
        VALUES (
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            COALESCE((SELECT request_count FROM wsp_runtime_status WHERE service_id = ?), 0),
            COALESCE((SELECT error_count FROM wsp_runtime_status WHERE service_id = ?), 0),
            COALESCE((SELECT restart_count FROM wsp_runtime_status WHERE service_id = ?), 0),
            COALESCE((SELECT last_health_ok FROM wsp_runtime_status WHERE service_id = ?), 0),
            ?,
            ?,
            ?,
            datetime('now')
        )
        ON CONFLICT(service_id) DO UPDATE SET
            pid = excluded.pid,
            status = excluded.status,
            cpu_percent = excluded.cpu_percent,
            memory_mb = excluded.memory_mb,
            gpu_memory_mb = excluded.gpu_memory_mb,
            request_count = COALESCE(excluded.request_count, wsp_runtime_status.request_count),
            error_count = COALESCE(excluded.error_count, wsp_runtime_status.error_count),
            restart_count = COALESCE(excluded.restart_count, wsp_runtime_status.restart_count),
            last_health_ok = COALESCE(excluded.last_health_ok, wsp_runtime_status.last_health_ok),
            started_at = COALESCE(excluded.started_at, wsp_runtime_status.started_at),
            stopped_at = excluded.stopped_at,
            error_message = excluded.error_message,
            updated_at = datetime('now')
        "#,
    )
    .bind(service_id)
    .bind(status.pid)
    .bind(final_status)
    .bind(status.cpu_percent)
    .bind(status.memory_mb)
    .bind(status.gpu_memory_mb)
    .bind(service_id)
    .bind(service_id)
    .bind(service_id)
    .bind(service_id)
    .bind(&status.started_at)
    .bind(&status.stopped_at)
    .bind(&status.error_message)
    .execute(pool)
    .await?;
    Ok(())
}

/// 记录 WSP 事件日志
pub async fn insert_event_log(pool: &SqlitePool, log: &WspEventLog) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO wsp_event_logs (id, service_id, event_type, pid, exit_code,
            crash_reason, cpu_percent, memory_mb, duration_ms, detail)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&log.id)
    .bind(&log.service_id)
    .bind(&log.event_type)
    .bind(log.pid)
    .bind(log.exit_code)
    .bind(&log.crash_reason)
    .bind(log.cpu_percent)
    .bind(log.memory_mb)
    .bind(log.duration_ms)
    .bind(&log.detail)
    .execute(pool)
    .await?;
    Ok(())
}

/// 获取 WSP 服务的事件日志
pub async fn get_event_logs(
    pool: &SqlitePool,
    service_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<WspEventLog>> {
    let rows = sqlx::query_as::<_, WspEventLog>(
        "SELECT * FROM wsp_event_logs WHERE service_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(service_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 获取所有启用的服务（按优先级排序）
pub async fn get_enabled_services(pool: &SqlitePool) -> anyhow::Result<Vec<WspService>> {
    let rows = sqlx::query_as::<_, WspService>(
        "SELECT id, name, display_name, description, version, category, \
         categoryName as category_name, subCategoryName as sub_category_name, \
         exe_path, work_dir, args, env_vars, protocol, host, port, base_path, ws_path, \
         health_endpoint, cpu_limit, memory_limit_mb, gpu_required, gpu_device_id, \
         auto_start, start_delay_ms, start_priority, restart_policy, max_restarts, \
         restart_cooldown_ms, health_check_interval_ms, health_check_timeout_ms, \
         startup_timeout_ms, depends_on, is_enabled, is_installed, sort_order, extra_config, created_at, updated_at \
         FROM wsp_services \
         WHERE is_enabled = 1 AND is_installed = 1 \
         ORDER BY start_priority ASC, sort_order ASC"
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 获取需要自动启动的服务（按优先级排序）
pub async fn get_auto_start_services(pool: &SqlitePool) -> anyhow::Result<Vec<WspService>> {
    let rows = sqlx::query_as::<_, WspService>(
        r#"
        SELECT id, name, display_name, description, version, category,
               categoryName as category_name, subCategoryName as sub_category_name,
               exe_path, work_dir, args, env_vars, protocol, host, port, base_path, ws_path,
               health_endpoint, cpu_limit, memory_limit_mb, gpu_required, gpu_device_id,
               auto_start, start_delay_ms, start_priority, restart_policy, max_restarts,
               restart_cooldown_ms, health_check_interval_ms, health_check_timeout_ms,
               startup_timeout_ms, depends_on, is_enabled, is_installed, sort_order, extra_config,
               created_at, updated_at
        FROM wsp_services
        WHERE auto_start = 1 AND is_enabled = 1 AND is_installed = 1
        ORDER BY start_priority ASC, start_delay_ms ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 记录 WSP 调用日志
pub async fn insert_call_log(pool: &SqlitePool, log: &WspCallLog) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO wsp_call_logs (id, service_id, endpoint_id, user_id, conversation_id,
            method, path, status_code, request_size, response_size, latency_ms, is_success, error_message)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&log.id).bind(&log.service_id).bind(&log.endpoint_id)
    .bind(&log.user_id).bind(&log.conversation_id)
    .bind(&log.method).bind(&log.path).bind(log.status_code)
    .bind(log.request_size).bind(log.response_size)
    .bind(log.latency_ms).bind(log.is_success).bind(&log.error_message)
    .execute(pool)
    .await?;
    Ok(())
}

/// 删除 WSP 服务（级联删除运行状态、日志、端点）
pub async fn delete_service(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM wsp_services WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
