mod types;

pub use types::*;

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

const MAX_HISTORY_SIZE: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobEvent {
    Queued(Job),
    Started {
        id: Uuid,
        rule_name: String,
    },
    Progress {
        id: Uuid,
        progress: f32,
        step: String,
    },
    Completed(Job),
    Failed {
        id: Uuid,
        error: String,
    },
}

pub struct AppState {
    jobs: RwLock<HashMap<Uuid, Job>>,
    queue: RwLock<VecDeque<Uuid>>,
    history: RwLock<VecDeque<Job>>,
    stats: RwLock<JobStats>,
    seen_files: RwLock<std::collections::HashSet<PathBuf>>,
    persistence_path: Option<PathBuf>,
    event_tx: broadcast::Sender<JobEvent>,
}

impl AppState {
    pub fn new(persistence_path: Option<PathBuf>) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(256);

        let state = Arc::new(Self {
            jobs: RwLock::new(HashMap::new()),
            queue: RwLock::new(VecDeque::new()),
            history: RwLock::new(VecDeque::new()),
            stats: RwLock::new(JobStats::default()),
            seen_files: RwLock::new(std::collections::HashSet::new()),
            persistence_path,
            event_tx,
        });

        // Try to load persisted state
        if let Some(ref path) = state.persistence_path {
            if let Err(e) = state.load_from_file(path) {
                tracing::warn!("Failed to load persisted state: {}", e);
            }
        }

        state
    }

    pub fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.event_tx.subscribe()
    }

    /// Queue a new job for processing
    pub fn queue_job(&self, file_path: PathBuf, source: JobSource) -> Result<Job> {
        {
            let mut seen = self.seen_files.write();
            if !seen.insert(file_path.clone()) {
                anyhow::bail!("File already being processed: {:?}", file_path);
            }
        }

        let job = Job::new(file_path.clone(), source);
        let id = job.id;

        {
            let mut jobs = self.jobs.write();
            jobs.insert(id, job.clone());
        }

        {
            let mut queue = self.queue.write();
            queue.push_back(id);
        }

        if self.event_tx.send(JobEvent::Queued(job.clone())).is_err() {
            tracing::debug!("No subscribers for job event");
        }
        self.persist();

        Ok(job)
    }

    /// Get the next job from the queue
    pub fn dequeue_job(&self) -> Option<Job> {
        let id = {
            let mut queue = self.queue.write();
            queue.pop_front()?
        };

        let jobs = self.jobs.read();
        jobs.get(&id).cloned()
    }

    /// Mark a job as started
    pub fn start_job(&self, id: Uuid, rule_name: &str) {
        let mut jobs = self.jobs.write();
        if let Some(job) = jobs.get_mut(&id) {
            job.start(rule_name);
            if self
                .event_tx
                .send(JobEvent::Started {
                    id,
                    rule_name: rule_name.to_string(),
                })
                .is_err()
            {
                tracing::debug!("No subscribers for job event");
            }
        }
        drop(jobs);
        self.persist();
    }

    /// Update job progress
    pub fn update_progress(&self, id: Uuid, progress: f32, step: &str) {
        let mut jobs = self.jobs.write();
        if let Some(job) = jobs.get_mut(&id) {
            job.update_progress(progress, step);
            if self
                .event_tx
                .send(JobEvent::Progress {
                    id,
                    progress,
                    step: step.to_string(),
                })
                .is_err()
            {
                tracing::debug!("No subscribers for job event");
            }
        }
    }

    /// Mark a job as completed
    pub fn complete_job(&self, id: Uuid) {
        let job = {
            let mut jobs = self.jobs.write();
            if let Some(job) = jobs.get_mut(&id) {
                job.complete();
                Some(job.clone())
            } else {
                None
            }
        };

        if let Some(job) = job {
            // Update stats
            {
                let mut stats = self.stats.write();
                let file_size = std::fs::metadata(&job.file_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                stats.record_success(job.rule_name.as_deref(), file_size);
            }

            // Remove from seen files
            {
                let mut seen = self.seen_files.write();
                seen.remove(&job.file_path);
            }

            // Move to history
            self.add_to_history(job.clone());

            // Remove from active jobs
            {
                let mut jobs = self.jobs.write();
                jobs.remove(&id);
            }

            if self.event_tx.send(JobEvent::Completed(job)).is_err() {
                tracing::debug!("No subscribers for job event");
            }
            self.persist();
        }
    }

    /// Mark a job as failed
    pub fn fail_job(&self, id: Uuid, error: &str) {
        let job = {
            let mut jobs = self.jobs.write();
            if let Some(job) = jobs.get_mut(&id) {
                job.fail(error);
                Some(job.clone())
            } else {
                None
            }
        };

        if let Some(job) = job {
            // Update stats
            {
                let mut stats = self.stats.write();
                stats.record_failure();
            }

            // Remove from seen files
            {
                let mut seen = self.seen_files.write();
                seen.remove(&job.file_path);
            }

            // Move to history
            self.add_to_history(job);

            // Remove from active jobs
            {
                let mut jobs = self.jobs.write();
                jobs.remove(&id);
            }

            if self
                .event_tx
                .send(JobEvent::Failed {
                    id,
                    error: error.to_string(),
                })
                .is_err()
            {
                tracing::debug!("No subscribers for job event");
            }
            self.persist();
        }
    }

    fn add_to_history(&self, job: Job) {
        let mut history = self.history.write();
        history.push_front(job);
        while history.len() > MAX_HISTORY_SIZE {
            history.pop_back();
        }
    }

    /// Get a job by ID
    pub fn get_job(&self, id: Uuid) -> Option<Job> {
        let jobs = self.jobs.read();
        jobs.get(&id).cloned()
    }

    /// Get all active jobs
    pub fn get_active_jobs(&self) -> Vec<Job> {
        let jobs = self.jobs.read();
        jobs.values().cloned().collect()
    }

    /// Get queued job IDs
    pub fn get_queue(&self) -> Vec<Uuid> {
        let queue = self.queue.read();
        queue.iter().cloned().collect()
    }

    /// Get job history
    pub fn get_history(&self, limit: usize) -> Vec<Job> {
        let history = self.history.read();
        history.iter().take(limit).cloned().collect()
    }

    /// Get stats
    pub fn get_stats(&self) -> JobStats {
        let stats = self.stats.read();
        stats.clone()
    }

    /// Retry a failed job
    pub fn retry_job(&self, id: Uuid) -> Result<Job> {
        // Find in history
        let job = {
            let history = self.history.read();
            history.iter().find(|j| j.id == id).cloned()
        };

        let job = job.ok_or_else(|| anyhow::anyhow!("Job not found in history"))?;

        if job.status != JobStatus::Failed {
            anyhow::bail!("Can only retry failed jobs");
        }

        // Queue a new job with the same file
        self.queue_job(job.file_path, job.source)
    }

    /// Delete a job from history
    pub fn delete_job(&self, id: Uuid) -> bool {
        let mut history = self.history.write();
        let len_before = history.len();
        history.retain(|j| j.id != id);
        let deleted = history.len() < len_before;
        drop(history);

        if deleted {
            self.persist();
        }
        deleted
    }

    fn persist(&self) {
        if let Some(ref path) = self.persistence_path {
            if let Err(e) = self.save_to_file(path) {
                tracing::error!("Failed to persist state: {}", e);
            }
        }
    }

    fn save_to_file(&self, path: &Path) -> Result<()> {
        #[derive(Serialize)]
        struct PersistedState {
            history: Vec<Job>,
            stats: JobStats,
        }

        let state = PersistedState {
            history: self.get_history(MAX_HISTORY_SIZE),
            stats: self.get_stats(),
        };

        let json = serde_json::to_string_pretty(&state)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    fn load_from_file(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        #[derive(Deserialize)]
        struct PersistedState {
            history: Vec<Job>,
            stats: JobStats,
        }

        let content = std::fs::read_to_string(path)?;
        let state: PersistedState = serde_json::from_str(&content)?;

        {
            let mut history = self.history.write();
            *history = VecDeque::from(state.history);
        }

        {
            let mut stats = self.stats.write();
            *stats = state.stats;
        }

        Ok(())
    }
}
