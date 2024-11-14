use std::{future::Future, sync::Arc};

use ahitool::{apis::job_nimbus, jobs::Job};
use chrono::{DateTime, Utc};
use tracing::{trace, warn};

use crate::{data_loader::DataLoader, resource};

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
            if self.data.fetch_in_progress() {
                ui.label("Fetching...");
            } else if ui.button("Fetch Jobs").clicked() {
                resource::runtime().spawn(self.fetch());
            }
            ui.label(format!(
                "Last fetched: {}",
                self.data.get_mut()
                    .as_ref()
                    .map(|d| d.fetched.time().to_string())
                    .as_deref()
                    .unwrap_or("never")
            ));
        });
        if let Some(data) = self.data.get_mut().as_ref() {
            ui.label(format!("{} jobs in memory", data.jobs.len()));
        }
    }

    fn fetch(&mut self) -> impl Future<Output = ()> {
        // Clone all the data we need up front, so that the resulting future
        // has no lifetime dependencies on self.
        let data_tx = self.data.start_fetch();
        let api_key = self.api_key.clone();

        async move {
            trace!("a");
            let answer = match job_nimbus::get_all_jobs_from_job_nimbus(resource::client(), &api_key, None).await {
                Ok(jobs) => {
                    let now = Utc::now();
                    trace!("b");
                    let jobs = jobs.map(|job| Arc::new(job)).collect();
                    Some(Arc::new(JobNimbusData { fetched: now, jobs }))
                }
                Err(e) => {
                    warn!("error fetching jobs: {}", e);
                    None
                }
            };
            trace!("data retrieved; sending back to client");
            let _ = data_tx.send(answer);
        }

    }
}
