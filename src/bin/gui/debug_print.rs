use tracing::{info, warn};

use crate::{
    data_loader::DataLoader,
    job_nimbus_client::{JobNimbusClient, JobNimbusData},
};

use std::{io::Write, sync::Arc, thread};

#[derive(Default)]
pub struct DebugPrint {
    pub output_file: String,
    printing: DataLoader<()>,
}

impl DebugPrint {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&mut self, ui: &mut egui::Ui, jn_client: &mut JobNimbusClient) {
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| jn_client.render(ui));

        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Debug Print");

            ui.horizontal(|ui| {
                ui.label("Output File:");
                ui.text_edit_singleline(&mut self.output_file);
            });

            if let Some(job_nimbus_data) = jn_client.get_data().as_ref() {
                let fetch_in_progress = self.printing.fetch_in_progress();
                let button = ui.add_enabled(
                    !fetch_in_progress && !self.output_file.is_empty(),
                    egui::Button::new("Print"),
                );
                if fetch_in_progress {
                    ui.label("Printing...");
                } else {
                    if button.clicked() {
                        self.start_print(job_nimbus_data.clone());
                    }
                }
            } else {
                ui.label("Fetch data first to print jobs.");
            }
        });
    }

    fn start_print(&mut self, data: Arc<JobNimbusData>) {
        let output_file = self.output_file.clone();
        info!("Starting to print job data to {}", &output_file);
        let completion_tx = self.printing.start_fetch();
        thread::spawn(move || {
            if let Ok(mut file) = std::fs::File::create(&output_file) {
                if let Err(e) = writeln!(file, "{:?}", &data.jobs) {
                    warn!("Failed to write to file: {}", e);
                } else {
                    info!("Finished printing job data to {}", &output_file)
                }
            } else {
                warn!("Failed to create file: {}", &output_file);
            }
            let _ = completion_tx.send(());
        });
    }
}
