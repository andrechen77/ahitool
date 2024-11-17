use std::sync::Arc;

use ahitool::tools::{
    self,
    kpi::{JobTrackerStats, KpiData, KpiSubject},
};
use tracing::warn;

use crate::{data_loader::DataLoader, job_nimbus_client::JobNimbusClient, resource};

#[derive(Default)]
pub struct KpiPage {
    pub kpi_data: DataLoader<Option<Arc<KpiData>>>,
    pub selected_rep: Option<KpiSubject>,
    pub spreadsheet_id: String,
    pub export_data: DataLoader<()>,
}

impl KpiPage {
    pub fn render(&mut self, ui: &mut egui::Ui, jn_client: &mut JobNimbusClient) {
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| jn_client.render(ui));
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Key Performance Indicators");
            if let Some(jn_data) = jn_client.data.get_mut().as_ref() {
                if ui.button("Calculate KPIs").clicked() {
                    let jn_data = Arc::clone(jn_data);
                    let kpi_data_tx = self.kpi_data.start_fetch();
                    resource::runtime().spawn(async move {
                        let kpi_data =
                            tools::kpi::calculate_kpi(jn_data.jobs.iter().cloned(), (None, None));
                        let _ = kpi_data_tx.send(Some(Arc::new(kpi_data)));
                    });
                }

                if let Some(kpi_data) = self.kpi_data.get_mut().as_ref() {
                    render_stats_viewer(ui, &mut self.selected_rep, kpi_data);
                } else {
                    ui.label("No KPI data available; use the button to calculate.");
                }
            } else {
                ui.label("No JobNimbus data available; use the button to fetch");
                return;
            }
        });
        let kpi_data = self.kpi_data.get_mut();
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Export to Google Sheets");
            ui.horizontal(|ui| {
                ui.label("Spreadsheet ID (empty to create):");
                ui.text_edit_singleline(&mut self.spreadsheet_id);
            });
            ui.horizontal(|ui| {
                let fetch_in_progress = self.export_data.fetch_in_progress();
                let button = ui.add_enabled(
                    kpi_data.is_some() && !fetch_in_progress,
                    egui::Button::new("Export"),
                );
                if fetch_in_progress {
                    ui.label("Exporting...");
                }
                if button.clicked() {
                    let spreadsheet_id =
                        Some(self.spreadsheet_id.clone()).filter(|s| !s.is_empty());
                    if let Some(kpi_data) = kpi_data.as_ref().cloned() {
                        let export_complete_tx = self.export_data.start_fetch();
                        resource::runtime().spawn_blocking(move || {
                            if let Err(err) = tools::kpi::output::generate_report_google_sheets(
                                &kpi_data,
                                spreadsheet_id.as_deref(),
                            ) {
                                warn!("Error exporting to Google Sheets: {}", err);
                            }
                            let _ = export_complete_tx.send(());
                        });
                    }
                }
            });
        });
    }
}

fn render_stats_viewer(
    ui: &mut egui::Ui,
    selected_rep: &mut Option<KpiSubject>,
    kpi_data: &KpiData,
) {
    // display and allow user to choose current tracker
    let heading =
        ui.label(selected_rep.as_ref().map_or("No rep selected (click me)", |rep| rep.as_str()));
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
                        *selected_rep = Some(rep.clone());
                    }
                }
            });
        },
    );

    // display the stats for the selected rep
    if let Some(selected_rep) = selected_rep.as_ref() {
        if let Some(stats) = kpi_data.stats_by_rep.get(selected_rep) {
            render_kpi_stats_table(ui, stats);
        } else {
            ui.label("No stats available for selected rep");
        }
    }
}

fn render_kpi_stats_table(ui: &mut egui::Ui, stats: &JobTrackerStats) {
    egui::Frame::none().stroke(egui::Stroke::new(1.0, egui::Color32::WHITE)).show(ui, |ui| {
        egui::Grid::new("stats table").num_columns(4).show(ui, |ui| {
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
