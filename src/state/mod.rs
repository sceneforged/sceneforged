mod types;

pub use types::*;

use anyhow::Result;
use parking_lot::RwLock;
use sceneforged_db::models::{Item, Library};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

const MAX_HISTORY_SIZE: usize = 1000;

/// Event category for filtering events by audience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventCategory {
    /// Admin-only events (job processing, system status).
    Admin,
    /// User-facing events (library changes, playback availability).
    User,
}

/// Application-wide event for SSE broadcasting.
///
/// This encompasses all event types that can be broadcast to connected clients.
/// Events are categorized as either "admin" (for administrative dashboards) or
/// "user" (for end-user clients).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AppEvent {
    // ========================================================================
    // Job Events (Admin category)
    // ========================================================================
    /// A job has been queued for processing.
    JobQueued {
        #[serde(flatten)]
        job: Job,
        category: EventCategory,
    },
    /// A job has started processing.
    JobStarted {
        id: Uuid,
        rule_name: String,
        category: EventCategory,
    },
    /// A job's progress has been updated.
    JobProgress {
        id: Uuid,
        progress: f32,
        step: String,
        category: EventCategory,
    },
    /// A job has completed successfully.
    JobCompleted {
        #[serde(flatten)]
        job: Job,
        category: EventCategory,
    },
    /// A job has failed.
    JobFailed {
        id: Uuid,
        error: String,
        category: EventCategory,
    },

    // ========================================================================
    // Library Events (User category)
    // ========================================================================
    /// A library scan has started.
    LibraryScanStarted {
        library_id: String,
        category: EventCategory,
    },
    /// A library scan has completed.
    LibraryScanComplete {
        library_id: String,
        items_added: u32,
        category: EventCategory,
    },
    /// A new library has been created.
    LibraryCreated {
        #[serde(flatten)]
        library: Library,
        category: EventCategory,
    },
    /// A library has been deleted.
    LibraryDeleted {
        library_id: String,
        category: EventCategory,
    },

    // ========================================================================
    // Item Events (User category)
    // ========================================================================
    /// A new item has been added to the library.
    ItemAdded {
        #[serde(flatten)]
        item: Item,
        category: EventCategory,
    },
    /// An item has been updated.
    ItemUpdated {
        #[serde(flatten)]
        item: Item,
        category: EventCategory,
    },
    /// An item has been removed from the library.
    ItemRemoved {
        item_id: String,
        category: EventCategory,
    },
    /// Playback is now available for an item (conversion completed or already playable).
    PlaybackAvailable {
        item_id: String,
        category: EventCategory,
    },

    // ========================================================================
    // Conversion Job Events (Admin category)
    // ========================================================================
    /// A conversion job has been created.
    ConversionJobCreated {
        job_id: String,
        item_id: String,
        status: String,
        category: EventCategory,
    },
    /// A conversion job has been cancelled.
    ConversionJobCancelled {
        job_id: String,
        item_id: String,
        category: EventCategory,
    },
}

impl AppEvent {
    /// Get the category of this event.
    pub fn category(&self) -> EventCategory {
        match self {
            // Job events are admin-only
            AppEvent::JobQueued { category, .. } => *category,
            AppEvent::JobStarted { category, .. } => *category,
            AppEvent::JobProgress { category, .. } => *category,
            AppEvent::JobCompleted { category, .. } => *category,
            AppEvent::JobFailed { category, .. } => *category,
            // Library events are user-facing
            AppEvent::LibraryScanStarted { category, .. } => *category,
            AppEvent::LibraryScanComplete { category, .. } => *category,
            AppEvent::LibraryCreated { category, .. } => *category,
            AppEvent::LibraryDeleted { category, .. } => *category,
            // Item events are user-facing
            AppEvent::ItemAdded { category, .. } => *category,
            AppEvent::ItemUpdated { category, .. } => *category,
            AppEvent::ItemRemoved { category, .. } => *category,
            AppEvent::PlaybackAvailable { category, .. } => *category,
            // Conversion job events are admin-only
            AppEvent::ConversionJobCreated { category, .. } => *category,
            AppEvent::ConversionJobCancelled { category, .. } => *category,
        }
    }

    /// Create a JobQueued event.
    pub fn job_queued(job: Job) -> Self {
        AppEvent::JobQueued {
            job,
            category: EventCategory::Admin,
        }
    }

    /// Create a JobStarted event.
    pub fn job_started(id: Uuid, rule_name: String) -> Self {
        AppEvent::JobStarted {
            id,
            rule_name,
            category: EventCategory::Admin,
        }
    }

    /// Create a JobProgress event.
    pub fn job_progress(id: Uuid, progress: f32, step: String) -> Self {
        AppEvent::JobProgress {
            id,
            progress,
            step,
            category: EventCategory::Admin,
        }
    }

    /// Create a JobCompleted event.
    pub fn job_completed(job: Job) -> Self {
        AppEvent::JobCompleted {
            job,
            category: EventCategory::Admin,
        }
    }

    /// Create a JobFailed event.
    pub fn job_failed(id: Uuid, error: String) -> Self {
        AppEvent::JobFailed {
            id,
            error,
            category: EventCategory::Admin,
        }
    }

    /// Create a LibraryScanStarted event.
    pub fn library_scan_started(library_id: String) -> Self {
        AppEvent::LibraryScanStarted {
            library_id,
            category: EventCategory::User,
        }
    }

    /// Create a LibraryScanComplete event.
    pub fn library_scan_complete(library_id: String, items_added: u32) -> Self {
        AppEvent::LibraryScanComplete {
            library_id,
            items_added,
            category: EventCategory::User,
        }
    }

    /// Create a LibraryCreated event.
    pub fn library_created(library: Library) -> Self {
        AppEvent::LibraryCreated {
            library,
            category: EventCategory::User,
        }
    }

    /// Create a LibraryDeleted event.
    pub fn library_deleted(library_id: String) -> Self {
        AppEvent::LibraryDeleted {
            library_id,
            category: EventCategory::User,
        }
    }

    /// Create an ItemAdded event.
    pub fn item_added(item: Item) -> Self {
        AppEvent::ItemAdded {
            item,
            category: EventCategory::User,
        }
    }

    /// Create an ItemUpdated event.
    pub fn item_updated(item: Item) -> Self {
        AppEvent::ItemUpdated {
            item,
            category: EventCategory::User,
        }
    }

    /// Create an ItemRemoved event.
    pub fn item_removed(item_id: String) -> Self {
        AppEvent::ItemRemoved {
            item_id,
            category: EventCategory::User,
        }
    }

    /// Create a PlaybackAvailable event.
    pub fn playback_available(item_id: String) -> Self {
        AppEvent::PlaybackAvailable {
            item_id,
            category: EventCategory::User,
        }
    }

    /// Create a ConversionJobCreated event.
    pub fn conversion_job_created(job_id: String, item_id: String, status: String) -> Self {
        AppEvent::ConversionJobCreated {
            job_id,
            item_id,
            status,
            category: EventCategory::Admin,
        }
    }

    /// Create a ConversionJobCancelled event.
    pub fn conversion_job_cancelled(job_id: String, item_id: String) -> Self {
        AppEvent::ConversionJobCancelled {
            job_id,
            item_id,
            category: EventCategory::Admin,
        }
    }
}

/// Legacy JobEvent type alias for backwards compatibility within the crate.
/// New code should use AppEvent directly.
pub type JobEvent = AppEvent;

pub struct AppState {
    jobs: RwLock<HashMap<Uuid, Job>>,
    queue: RwLock<VecDeque<Uuid>>,
    history: RwLock<VecDeque<Job>>,
    stats: RwLock<JobStats>,
    seen_files: RwLock<std::collections::HashSet<PathBuf>>,
    persistence_path: Option<PathBuf>,
    event_tx: broadcast::Sender<AppEvent>,
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

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.event_tx.subscribe()
    }

    /// Get a clone of the event sender for use in other components.
    pub fn event_sender(&self) -> broadcast::Sender<AppEvent> {
        self.event_tx.clone()
    }

    /// Broadcast an event to all subscribers.
    pub fn broadcast(&self, event: AppEvent) {
        if self.event_tx.send(event).is_err() {
            tracing::debug!("No subscribers for event");
        }
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

        if self.event_tx.send(AppEvent::job_queued(job.clone())).is_err() {
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
                .send(AppEvent::job_started(id, rule_name.to_string()))
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
                .send(AppEvent::job_progress(id, progress, step.to_string()))
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

            if self.event_tx.send(AppEvent::job_completed(job)).is_err() {
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
                .send(AppEvent::job_failed(id, error.to_string()))
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
