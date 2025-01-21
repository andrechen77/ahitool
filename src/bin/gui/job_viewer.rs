use tracing::warn;

use crate::job_nimbus_client::JobNimbusClient;

use std::io::Write;

#[derive(Default)]
pub struct JobNimbusViewer {
    pub output_file: String,
}

impl JobNimbusViewer {
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

            if let Some(job_nimbus_data) = jn_client.get_data().as_deref() {
                if ui.button("Print").clicked() {
                    if let Ok(mut file) = std::fs::File::create(&self.output_file) {
                        if let Err(e) = writeln!(file, "{:?}", &job_nimbus_data.jobs) {
                            warn!("Failed to write to file: {}", e);
                        }
                    } else {
                        warn!("Failed to create file: {}", self.output_file);
                    }
                }
            } else {
                ui.label("Fetch data first to print jobs.");
            }
        });
    }
}
