use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub file_path: PathBuf,
    pub file_name: String,
    pub status: JobStatus,
    pub rule_name: Option<String>,
    pub progress: f32,
    pub current_step: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub source: JobSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobSource {
    Webhook {
        arr_name: String,
        /// Movie ID (Radarr) or Series ID (Sonarr) for callbacks
        item_id: Option<i64>,
    },
    Watcher {
        watch_path: String,
    },
    Manual,
    Api,
}

impl Job {
    pub fn new(file_path: PathBuf, source: JobSource) -> Self {
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            id: Uuid::new_v4(),
            file_path,
            file_name,
            status: JobStatus::Queued,
            rule_name: None,
            progress: 0.0,
            current_step: None,
            error: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            source,
        }
    }

    pub fn start(&mut self, rule_name: &str) {
        self.status = JobStatus::Running;
        self.rule_name = Some(rule_name.to_string());
        self.started_at = Some(Utc::now());
    }

    pub fn update_progress(&mut self, progress: f32, step: &str) {
        self.progress = progress.clamp(0.0, 100.0);
        self.current_step = Some(step.to_string());
    }

    pub fn complete(&mut self) {
        self.status = JobStatus::Completed;
        self.progress = 100.0;
        self.current_step = None;
        self.completed_at = Some(Utc::now());
    }

    pub fn fail(&mut self, error: &str) {
        self.status = JobStatus::Failed;
        self.error = Some(error.to_string());
        self.completed_at = Some(Utc::now());
    }

    pub fn cancel(&mut self) {
        self.status = JobStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobStats {
    pub total_processed: u64,
    pub successful: u64,
    pub failed: u64,
    pub total_bytes_processed: u64,
    pub rules_matched: std::collections::HashMap<String, u64>,
}

impl JobStats {
    pub fn success_rate(&self) -> f32 {
        if self.total_processed == 0 {
            return 0.0;
        }
        (self.successful as f32 / self.total_processed as f32) * 100.0
    }

    pub fn record_success(&mut self, rule_name: Option<&str>, bytes: u64) {
        self.total_processed += 1;
        self.successful += 1;
        self.total_bytes_processed += bytes;
        if let Some(name) = rule_name {
            *self.rules_matched.entry(name.to_string()).or_insert(0) += 1;
        }
    }

    pub fn record_failure(&mut self) {
        self.total_processed += 1;
        self.failed += 1;
    }
}
