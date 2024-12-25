use std::{sync::Arc, thread};

use ahitool::{apis::job_nimbus, jobs::Job};
use chrono::{DateTime, Utc};
use tracing::{trace, warn};

use crate::data_loader::DataLoader;

pub struct JobNimbusData {
    pub fetched: DateTime<Utc>,
    pub jobs: Vec<Arc<Job>>,
}

#[derive(Default)]
pub struct JobNimbusClient {
    pub show_api_key: bool,
    /// The API key used to fetch from JobNimbus.
    pub api_key: String,
    /// The data fetched from JobNimbus.
    pub data: DataLoader<Option<Arc<JobNimbusData>>>,
}

impl JobNimbusClient {
    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.heading("JobNimbus");

        ui.horizontal(|ui| {
            ui.label("JobNimbus API Key:");
            ui.checkbox(&mut self.show_api_key, "show");
            if self.show_api_key {
                ui.text_edit_singleline(&mut self.api_key);
            } else {
                ui.text_edit_singleline(&mut "************");
            }
        });

        ui.horizontal(|ui| {
            let fetch_in_progress = self.data.fetch_in_progress();
            let button = ui.add_enabled(!fetch_in_progress, egui::Button::new("Fetch jobs"));
            if fetch_in_progress {
                ui.label("Fetching...");
            } else {
                ui.label(format!(
                    "Last fetched: {}",
                    self.data
                        .get_mut()
                        .as_ref()
                        .map(|d| d.fetched.time().to_string())
                        .as_deref()
                        .unwrap_or("never")
                ));
            }
            if button.clicked() {
                self.start_fetch();
            }
        });
        if let Some(data) = self.data.get_mut().as_ref() {
            ui.label(format!("{} jobs in memory", data.jobs.len()));
        }
    }

    /// Starts a fetch running on a separate thread. The data will be available
    /// in `self.data`.
    fn start_fetch(&mut self) {
        // Clone all the data we need up front, so that the resulting future
        // has no lifetime dependencies on self.
        let data_tx = self.data.start_fetch();
        let api_key = self.api_key.clone();

        thread::spawn(move || {
            let answer = match job_nimbus::get_all_jobs_from_job_nimbus(&api_key, None) {
                Ok(jobs) => {
                    let now = Utc::now();
                    let jobs = jobs.map(|job| Arc::new(job)).collect();
                    Some(Arc::new(JobNimbusData { fetched: now, jobs }))
                }
                Err(e) => {
                    warn!("error fetching jobs: {}", e);
                    None
                }
            };
            trace!("fetch complete; sending results back to UI component");
            let _ = data_tx.send(answer);
        });
    }
}
