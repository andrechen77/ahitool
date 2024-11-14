use std::sync::Arc;

use ahitool::tools::{self, kpi::{JobTrackerStats, KpiData, KpiSubject}};

use crate::{data_loader::DataLoader, job_nimbus_client::JobNimbusClient, resource};

#[derive(Default)]
pub struct KpiPage {
    kpi_data: DataLoader<Option<KpiData>>,
    selected_rep: Option<KpiSubject>,
}

impl KpiPage {
    pub fn render(&mut self, ui: &mut egui::Ui, jn_client: &mut JobNimbusClient) {
        egui::Frame::none().show(ui, |ui| jn_client.render(ui));
        ui.separator();
        egui::Frame::none().show(ui, |ui| {
            if let Some(jn_data) = jn_client.data.get_mut().as_ref() {
                if ui.button("calculate KPIs").clicked() {
                    let jn_data = Arc::clone(jn_data);
                    let kpi_data_tx = self.kpi_data.start_fetch();
                    resource::runtime().spawn(async move {
                        let kpi_data = tools::kpi::calculate_kpi(jn_data.jobs.iter().cloned(), (None, None));
                        let _ = kpi_data_tx.send(Some(kpi_data));
                    });
                }

                if let Some(kpi_data) = self.kpi_data.get_mut().as_ref() {
                    // display and allow user to choose current tracker
                    let heading = ui.label(self.selected_rep.as_ref().map_or("no rep selected", |rep| rep.as_str()));
                    let popup_id = ui.make_persistent_id("rep_chooser");
                    if heading.clicked() {
                        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                    }
                    egui::popup_above_or_below_widget(
                        ui,
                        popup_id,
                        &heading,
                        egui::AboveOrBelow::Below,
                        egui::PopupCloseBehavior::CloseOnClick,
                        |ui| {
                            ui.set_min_width(200.0);
                            ui.label("Choose a sales rep:");
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for (rep, _) in &kpi_data.stats_by_rep {
                                    if ui.button(rep.as_str()).clicked() {
                                        self.selected_rep = Some(rep.clone());
                                    }
                                }
                            });
                        },
                    );

                    // display the stats for the selected rep
                    if let Some(selected_rep) = self.selected_rep.as_ref() {
                        if let Some(stats) = kpi_data.stats_by_rep.get(selected_rep) {
                            render_kpi_stats_table(ui, stats);
                        } else {
                            ui.label("No stats available for selected rep");
                        }
                    }
                } else {
                    ui.label("No KPI data available; use the button to calculate.");
                }
            } else {
                ui.label("No JobNimbus data available; use the button to fetch");
                return;
            }
        });
    }
}


fn render_kpi_stats_table(ui: &mut egui::Ui, stats: &JobTrackerStats) {
    egui::Frame::none().stroke(egui::Stroke::new(1.0, egui::Color32::WHITE)).show(ui, |ui| {
        egui::Grid::new("stats table")
            .num_columns(4)
            .show(ui, |ui| {
                ui.label("Conversion");
                ui.label("Rate");
                ui.label("Total");
                ui.label("Average Time (days)");
                ui.end_row();

                for (name, conv_stats) in [
                    ("All Losses", &stats.loss_conv),
                    ("(I) Appt to Contingency", &stats.appt_continge_conv),
                    ("(I) Appt to Contract", &stats.appt_contract_insure_conv),
                    ("(I) Contingency to Contract", &stats.continge_contract_conv),
                    ("(R) Appt to Contract", &stats.appt_contract_retail_conv),
                    ("(I) Contract to Installation", &stats.install_insure_conv),
                    ("(R) Contract to Installation", &stats.install_retail_conv),
                ] {
                    use tools::kpi::output;
                    ui.label(name);
                    ui.label(&output::percent_or_na(conv_stats.conversion_rate));
                    ui.label(&conv_stats.achieved.len().to_string());
                    ui.label(format!("{:.2}", output::into_days(conv_stats.average_time_to_achieve)));
                    ui.end_row();
                }
            });
    });
}
