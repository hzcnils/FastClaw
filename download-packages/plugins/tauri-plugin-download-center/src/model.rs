use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadInput {
    pub resource_type: String,
    pub resource_id: String,
    pub name: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub silent: bool,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub task_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadExecutionPlan {
    pub instructions: Vec<DownloadInstruction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DownloadInstruction {
    Download {
        url: String,
        temp_path: String,
        #[serde(default = "default_true")]
        resume: bool,
    },
    VerifySha256 {
        sha256: String,
    },
    ExtractZip {
        staging_path: String,
        required_file_suffix: Option<String>,
    },
    BackupTarget {
        target_path: String,
        backup_path: Option<String>,
    },
    ReplaceTarget {
        source: ReplaceSource,
        target_path: String,
    },
    OverlayTarget {
        source: ReplaceSource,
        target_path: String,
    },
    Cleanup {
        targets: Vec<CleanupTarget>,
    },
}
fn default_true() -> bool {
    true
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReplaceSource {
    Temp,
    Staging,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupTarget {
    Temp,
    Staging,
    Backup,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadError {
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTaskRow {
    pub id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub name: String,
    pub status: String,
    pub phase: String,
    pub progress: i64,
    pub downloaded_bytes: i64,
    pub total_bytes: Option<i64>,
    pub message_key: Option<String>,
    #[serde(skip_serializing)]
    pub message_params_json: Option<String>,
    #[serde(skip_serializing)]
    pub error_code: Option<String>,
    #[serde(skip_serializing)]
    pub error_message: Option<String>,
    #[serde(skip_serializing)]
    pub payload_json: String,
    #[serde(skip_serializing)]
    pub target_path: Option<String>,
    #[serde(skip_serializing)]
    pub temp_path: Option<String>,
    #[serde(skip_serializing)]
    pub etag: Option<String>,
    #[serde(skip_serializing)]
    pub last_modified: Option<String>,
    pub silent: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub name: String,
    pub status: String,
    pub phase: String,
    pub progress: i64,
    pub downloaded_bytes: i64,
    pub total_bytes: Option<i64>,
    pub message_key: Option<String>,
    pub message_params: Option<serde_json::Value>,
    pub error: Option<DownloadError>,
    pub silent: bool,
    pub payload: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}
impl From<DownloadTaskRow> for DownloadTask {
    fn from(row: DownloadTaskRow) -> Self {
        let error = row.error_message.as_ref().map(|message| DownloadError {
            code: row
                .error_code
                .clone()
                .unwrap_or_else(|| "DOWNLOAD_ERROR".into()),
            message: message.clone(),
        });
        Self {
            id: row.id,
            resource_type: row.resource_type,
            resource_id: row.resource_id,
            name: row.name,
            status: row.status,
            phase: row.phase,
            progress: row.progress,
            downloaded_bytes: row.downloaded_bytes,
            total_bytes: row.total_bytes,
            message_key: row.message_key,
            message_params: row
                .message_params_json
                .and_then(|value| serde_json::from_str(&value).ok()),
            error,
            silent: row.silent,
            payload: serde_json::from_str(&row.payload_json).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
