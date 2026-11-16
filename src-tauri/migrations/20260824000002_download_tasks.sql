DROP TABLE IF EXISTS download_tasks;
CREATE TABLE IF NOT EXISTS package_download_tasks (
 id TEXT PRIMARY KEY NOT NULL,resource_type TEXT NOT NULL,resource_id TEXT NOT NULL,name TEXT NOT NULL,status TEXT NOT NULL,phase TEXT NOT NULL DEFAULT '',progress INTEGER NOT NULL DEFAULT 0,downloaded_bytes INTEGER NOT NULL DEFAULT 0,total_bytes INTEGER,message_key TEXT,message_params_json TEXT,error_code TEXT,error_message TEXT,payload_json TEXT NOT NULL DEFAULT '{}',target_path TEXT,temp_path TEXT,etag TEXT,last_modified TEXT,silent INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_package_download_tasks_status ON package_download_tasks(status);
CREATE INDEX IF NOT EXISTS idx_package_download_tasks_resource ON package_download_tasks(resource_type,resource_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_package_download_tasks_active_resource ON package_download_tasks(resource_type,resource_id) WHERE status IN ('pending','downloading','pausing','paused','verifying','installing');
