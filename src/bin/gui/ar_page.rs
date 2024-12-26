use std::{sync::Arc, thread};

use ahitool::{
    tools::{self, acc_receivable::AccRecvableData},
    utils,
};
use tracing::warn;

use crate::{
    data_loader::DataLoader,
    job_nimbus_client::{JobNimbusClient, JobNimbusData},
};

#[derive(Default)]
pub struct ArPage {
    pub ar_data: DataLoader<Option<Arc<AccRecvableData>>>,
    pub spreadsheet_id: String,
    pub export_data: DataLoader<()>,
}

impl ArPage {
    pub fn render(&mut self, ui: &mut egui::Ui, jn_client: &mut JobNimbusClient) {
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| jn_client.render(ui));
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Accounts Receivable Report");
            if let Some(jn_data) = jn_client.data.get_mut().as_ref().cloned() {
                if ui.button("Calculate Accounts Receivable").clicked() {
                    self.start_calculate(jn_data);
                }

                if let Some(ar_data) = self.ar_data.get_mut().as_ref() {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        render_api_hierarchy(ui, ar_data);
                    });
                } else {
                    ui.label("No AR data available; use the button to calculate.");
                }
            } else {
                ui.label("No JobNimbus data available; use the button to fetch");
                return;
            }
        });
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Export to Google Sheets");
            ui.horizontal(|ui| {
                ui.label("Spreadsheet ID (empty to create):");
                ui.text_edit_singleline(&mut self.spreadsheet_id);
            });
            ui.horizontal(|ui| {
                let ar_data = self.ar_data.get_mut();
                let fetch_in_progress = self.export_data.fetch_in_progress();
                let button = ui.add_enabled(
                    ar_data.is_some() && !fetch_in_progress,
                    egui::Button::new("Export"),
                );
                if fetch_in_progress {
                    ui.label("Exporting...");
                }
                if button.clicked() {
                    let spreadsheet_id =
                        Some(self.spreadsheet_id.clone()).filter(|s| !s.is_empty());
                    if let Some(data) = ar_data.as_ref().map(|a| Arc::clone(a)) {
                        // stop borrowing self before we borrow it again to
                        // generate the google sheets
                        drop(ar_data);
                        self.start_generate_google_sheets(data, spreadsheet_id);
                    }
                }
            });
        });
    }

    fn start_calculate(&mut self, jn_data: Arc<JobNimbusData>) {
        let ar_data_tx = self.ar_data.start_fetch();
        thread::spawn(move || {
            let ar_data =
                tools::acc_receivable::calculate_acc_receivable(jn_data.jobs.iter().cloned());
            let _ = ar_data_tx.send(Some(Arc::new(ar_data)));
        });
    }

    fn start_generate_google_sheets(
        &mut self,
        ar_data: Arc<AccRecvableData>,
        spreadsheet_id: Option<String>,
    ) {
        let export_data_tx = self.export_data.start_fetch();
        thread::spawn(move || {
            if let Err(e) = tools::acc_receivable::generate_report_google_sheets(
                &ar_data,
                spreadsheet_id.as_deref(),
            ) {
                warn!("Error exporting to Google Sheets: {}", e);
            }
            let _ = export_data_tx.send(());
        });
    }
}

fn render_api_hierarchy(ui: &mut egui::Ui, ar_data: &AccRecvableData) {
    egui::CollapsingHeader::new(format!("Total: ${}", ar_data.total as f64 / 100.0))
        .default_open(false)
        .show(ui, |ui| {
            for (status, (category_total, jobs)) in &ar_data.categorized_jobs {
                egui::CollapsingHeader::new(format!(
                    "{}: total: ${}",
                    status,
                    *category_total as f64 / 100.0
                ))
                .default_open(false)
                .show(ui, |ui| {
                    for job in jobs {
                        let text = format!(
                            "{} owes ${:.2}",
                            job.job_number.as_deref().unwrap_or("[no job number]"),
                            job.amt_receivable as f64 / 100.0
                        );
                        let label = egui::Label::new(text).sense(egui::Sense::click());
                        if ui.add(label).clicked() {
                            utils::open_url(&format!("https://app.jobnimbus.com/job/{}", job.jnid));
                        }
                    }
                });
            }
        });
}
