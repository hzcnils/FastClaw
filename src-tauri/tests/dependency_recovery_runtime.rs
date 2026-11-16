//! 独立于主程序单元测试，验证同步 IPC 的后台恢复记录调度。

#[test]
fn recovery_publish_uses_tauri_runtime_for_sync_commands() {
    let source = include_str!("../src/core/dependency_services/manager.rs");
    let publish = source
        .split_once("fn publish(&self, app: &tauri::AppHandle)")
        .unwrap().1
        .split_once("pub fn is_current(").unwrap().0;
    assert!(publish.contains("tauri::async_runtime::spawn(async move"));
    assert!(!publish.lines().any(|line| line.trim_start().starts_with("tokio::spawn(")));
}

#[test]
fn recovery_record_task_can_run_from_thread_without_tokio_context() {
    // 普通线程模拟同步 Tauri 命令，不能用 #[tokio::test] 掩盖运行时缺失问题。
    std::thread::spawn(|| {
        assert!(tokio::runtime::Handle::try_current().is_err());
        let (sender, receiver) = std::sync::mpsc::channel();
        let task = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            sender.send(()).unwrap();
        });
        receiver.recv_timeout(std::time::Duration::from_secs(5))
            .expect("同步命令应成功调度后台任务，而不是因缺少 Tokio 上下文退出");
        tauri::async_runtime::block_on(task).unwrap();
    }).join().expect("同步命令线程不应 panic");
}
