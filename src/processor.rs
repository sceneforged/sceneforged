use sceneforged::arr;
use sceneforged::config::Config;
use sceneforged::notifications::NotificationManager;
use sceneforged::pipeline::PipelineExecutor;
use sceneforged::probe;
use sceneforged::rules;
use sceneforged::state::{AppState, Job, JobSource};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Job processor that runs pipelines for queued jobs
pub struct JobProcessor {
    state: Arc<AppState>,
    config: Arc<Config>,
    shutdown_rx: mpsc::Receiver<()>,
    notifications: NotificationManager,
}

impl JobProcessor {
    pub fn new(state: Arc<AppState>, config: Arc<Config>, shutdown_rx: mpsc::Receiver<()>) -> Self {
        let notifications = NotificationManager::new(&config);
        Self {
            state,
            config,
            shutdown_rx,
            notifications,
        }
    }

    /// Start processing jobs from the queue
    pub async fn run(mut self) {
        tracing::info!("Job processor started");

        loop {
            // Check for shutdown signal with a timeout
            tokio::select! {
                biased;

                _ = self.shutdown_rx.recv() => {
                    tracing::info!("Job processor shutting down");
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Continue to process jobs
                }
            }

            // Process next job if available
            self.process_next_job().await;
        }
    }

    async fn process_next_job(&self) {
        // Get next job from queue
        let job = match self.state.dequeue_job() {
            Some(job) => job,
            None => {
                // No jobs, wait a bit longer
                tokio::time::sleep(tokio::time::Duration::from_millis(900)).await;
                return;
            }
        };

        let job_id = job.id;
        let file_path = job.file_path.clone();
        let job_for_callback = job.clone();

        tracing::info!("Processing job {}: {:?}", job_id, file_path);

        // Process the job
        let result = self.process_file(job_id, &file_path).await;

        match result {
            Ok(_rule_name) => {
                tracing::info!("Job {} completed successfully", job_id);
                self.state.complete_job(job_id);

                // Trigger Arr callback (non-blocking, errors logged but not fatal)
                self.trigger_arr_callback(&job_for_callback).await;

                // Notify media servers (Jellyfin, etc.)
                self.notifications.notify_job_completed(&file_path).await;
            }
            Err(e) => {
                tracing::error!("Job {} failed: {}", job_id, e);
                self.state.fail_job(job_id, &e.to_string());
            }
        }
    }

    async fn process_file(
        &self,
        job_id: uuid::Uuid,
        file_path: &PathBuf,
    ) -> anyhow::Result<Option<String>> {
        // Probe the file
        let media_info = probe::probe_file(file_path)?;

        // Find matching rule
        let rule = rules::find_matching_rule(&media_info, &self.config.rules);

        let rule = match rule {
            Some(r) => r,
            None => {
                tracing::info!("No matching rule for: {:?}", file_path);
                return Ok(None);
            }
        };

        let rule_name = rule.name.clone();

        // Update job state to running
        self.state.start_job(job_id, &rule_name);

        tracing::info!("Matched rule: {} for {:?}", rule_name, file_path);

        // Execute the pipeline (blocking, wrapped in spawn_blocking)
        let actions = rule.actions.clone();
        let path = file_path.clone();
        let state = self.state.clone();
        let job_id_clone = job_id;

        let result = tokio::task::spawn_blocking(move || {
            let executor = PipelineExecutor::new(&path, false)?;

            // Set up progress callback
            let executor = executor.with_progress_callback(Box::new(move |progress, step| {
                state.update_progress(job_id_clone, progress, step);
            }));

            executor.execute(&actions)
        })
        .await??;

        tracing::info!("Pipeline completed: {:?}", result);
        Ok(Some(rule_name))
    }

    /// Trigger Arr callback (rescan) after successful job completion
    async fn trigger_arr_callback(&self, job: &Job) {
        // Only trigger for webhook-sourced jobs
        let (arr_name, item_id) = match &job.source {
            JobSource::Webhook { arr_name, item_id } => match item_id {
                Some(id) => (arr_name.clone(), *id),
                None => {
                    tracing::debug!(
                        "Job {} from {} has no item_id, skipping callback",
                        job.id,
                        arr_name
                    );
                    return;
                }
            },
            _ => return, // Not a webhook job
        };

        // Find the arr config
        let arr_config = match self
            .config
            .arrs
            .iter()
            .find(|a| a.name.to_lowercase() == arr_name.to_lowercase() && a.enabled)
        {
            Some(c) => c,
            None => {
                tracing::debug!(
                    "Arr '{}' not found or disabled, skipping callback",
                    arr_name
                );
                return;
            }
        };

        // Check if auto_rescan is enabled
        if !arr_config.auto_rescan {
            tracing::debug!("Auto-rescan disabled for '{}', skipping callback", arr_name);
            return;
        }

        // Create client and trigger rescan
        let client = arr::create_client(arr_config);

        tracing::info!(
            "Triggering rescan for {} item {} after job {}",
            arr_name,
            item_id,
            job.id
        );

        match client.rescan(item_id).await {
            Ok(()) => {
                tracing::info!(
                    "Successfully triggered rescan for {} item {}",
                    arr_name,
                    item_id
                );
            }
            Err(e) => {
                // Log error but don't fail the job - callback failure is non-fatal
                tracing::warn!(
                    "Failed to trigger rescan for {} item {}: {}",
                    arr_name,
                    item_id,
                    e
                );
            }
        }

        // Trigger rename if enabled (after rescan so it has updated file info)
        if arr_config.auto_rename {
            tracing::info!(
                "Triggering rename for {} item {} after job {}",
                arr_name,
                item_id,
                job.id
            );

            match client.rename(item_id).await {
                Ok(()) => {
                    tracing::info!(
                        "Successfully triggered rename for {} item {}",
                        arr_name,
                        item_id
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to trigger rename for {} item {}: {}",
                        arr_name,
                        item_id,
                        e
                    );
                }
            }
        }
    }
}
