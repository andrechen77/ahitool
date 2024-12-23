use ahitool::{tools::{self, acc_receivable::AccRecvableData}, utils};

use crate::{data_loader::DataLoader, job_nimbus_client::JobNimbusClient, resource};

#[derive(Default)]
pub struct ArPage {
    pub ar_data: DataLoader<Option<AccRecvableData>>,
}

impl ArPage {
    pub fn render(&mut self, ui: &mut egui::Ui, jn_client: &mut JobNimbusClient) {
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| jn_client.render(ui));
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Accounts Receivable Report");
            if let Some(jn_data) = jn_client.data.get_mut().as_ref().cloned() {
                if ui.button("Calculate Accounts Receivable").clicked() {
                    let ar_data_tx = self.ar_data.start_fetch();
                    resource::runtime().spawn(async move {
                        let ar_data = tools::acc_receivable::calculate_acc_receivable(
                            jn_data.jobs.iter().cloned(),
                        );
                        let _ = ar_data_tx.send(Some(ar_data));
                    });
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
                        let text = format!("{} owes ${:.2}", job.job_number.as_deref().unwrap_or("[no job number]"), job.amt_receivable as f64 / 100.0);
                        let label = egui::Label::new(text).sense(egui::Sense::click());
                        if ui.add(label).clicked() {
                            utils::open_url(&format!("https://app.jobnimbus.com/job/{}", job.jnid));
                        }
                    }
                });
            }
        });
}
