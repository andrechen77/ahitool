use std::{
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

use ahitool::{tools, utils::FileBacked};
use tracing::warn;

use crate::{
    data_loader::DataLoader,
    job_nimbus_client::{JobNimbusClient, JobNimbusData},
};

pub struct AllJobsPage {
    pub spreadsheet_id: FileBacked<String>,
    /// Tracks the progress of exporting the data to Google Sheets. The data
    /// is the id of the successfully exported spreadsheet.
    export_data: DataLoader<Option<String>>,
}

impl AllJobsPage {
    pub fn new(spreadsheet_id: FileBacked<String>) -> Self {
        Self { spreadsheet_id, export_data: DataLoader::new(None) }
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        jn_client: &mut JobNimbusClient,
        oauth_cache_file: &Path,
    ) {
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| jn_client.render(ui));
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Export all jobs to Google Sheets");
            ui.horizontal(|ui| {
                ui.label("Spreadsheet ID (empty to create):");
                let spreadsheet_id = self.spreadsheet_id.get_mut();
                if let Some(new_spreadsheet_id) = self.export_data.get_mut().take() {
                    *spreadsheet_id = new_spreadsheet_id;
                }
                ui.text_edit_singleline(spreadsheet_id);
            });
            ui.horizontal(|ui| {
                let jn_data = jn_client.get_data();
                let fetch_in_progress = self.export_data.fetch_in_progress();
                let button = ui.add_enabled(
                    jn_data.is_some() && !fetch_in_progress,
                    egui::Button::new("Export"),
                );
                if fetch_in_progress {
                    ui.label("Exporting...");
                }
                if button.clicked() {
                    let spreadsheet_id =
                        Some(self.spreadsheet_id.get().clone()).filter(|s| !s.is_empty());
                    if let Some(data) = jn_data.as_ref().map(|a| Arc::clone(a)) {
                        // stop borrowing self before we borrow it again to
                        // generate the google sheets
                        drop(jn_data);
                        self.start_generate_google_sheets(
                            data,
                            spreadsheet_id,
                            oauth_cache_file.to_path_buf(),
                        );
                    }
                }
            });
        });
    }

    pub fn start_generate_google_sheets(
        &mut self,
        jn_data: Arc<JobNimbusData>,
        spreadsheet_id: Option<String>,
        oauth_cache_file: PathBuf,
    ) {
        let export_complete_tx = self.export_data.start_fetch();
        thread::spawn(move || {
            let new_spreadsheet_id = tools::all_jobs::generate_all_jobs_google_sheets(
                jn_data.jobs.iter().cloned(),
                spreadsheet_id.as_deref(),
                &oauth_cache_file,
            )
            .inspect_err(|err| {
                warn!("Error exporting to Google Sheets: {}", err);
            })
            .ok();
            let _ = export_complete_tx.send(new_spreadsheet_id);
        });
    }

    pub fn on_exit(&mut self) {
        if let Err(e) = self.spreadsheet_id.write_back() {
            warn!("error writing spreadsheet ID to cache file: {}", e);
        }
    }
}
