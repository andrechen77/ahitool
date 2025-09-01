use std::{sync::Arc, thread};

use ahitool::{
    date_range::DateRange,
    tools::{
        self,
        kpi::{JobTrackerStats, KpiData, KpiSubject},
    },
    utils::FileBacked,
};
use tracing::warn;

use crate::{
    data_loader::DataLoader,
    job_nimbus_client::{JobNimbusClient, JobNimbusData},
};

pub struct KpiPage {
    pub selected_rep: Option<KpiSubject>,
    pub spreadsheet_id: FileBacked<String>,
    /// The current value of the date range dropdown selector.
    date_range_option: (DateRangeOption, DateRangeOption),
    /// The current value of the date range custom date fields.
    date_range_custom: (String, String),
    /// The current value of the lead source dropdown selector.
    lead_source_option: Option<String>,
    /// The current value of the branch dropdown selector
    branch: Option<String>,
    kpi_data: DataLoader<Option<Arc<KpiData>>>,
    /// Tracks the progress of exporting the data to Google Sheets. The data
    /// is the id of the successfully exported spreadsheet.
    export_data: DataLoader<Option<String>>,
}

impl KpiPage {
    pub fn new(spreadsheet_id: FileBacked<String>) -> Self {
        Self {
            selected_rep: None,
            spreadsheet_id,
            date_range_option: (DateRangeOption::Forever, DateRangeOption::Today),
            date_range_custom: (String::new(), String::new()),
            lead_source_option: None,
            branch: None,
            kpi_data: DataLoader::new(None),
            export_data: DataLoader::new(None),
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, jn_client: &mut JobNimbusClient) {
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| jn_client.render(ui));
        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Calculate Key Performance Indicators");

            if let Some(jn_data) = jn_client.get_data().as_ref() {
                egui::ComboBox::from_label("From date")
                    .selected_text(self.date_range_option.0.to_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.date_range_option.0,
                            DateRangeOption::Forever,
                            DateRangeOption::Forever.to_str(),
                        );
                        ui.selectable_value(
                            &mut self.date_range_option.0,
                            DateRangeOption::StartOfYear,
                            DateRangeOption::StartOfYear.to_str(),
                        );
                        ui.selectable_value(
                            &mut self.date_range_option.0,
                            DateRangeOption::Custom,
                            DateRangeOption::Custom.to_str(),
                        );
                    });
                if self.date_range_option.0 == DateRangeOption::Custom {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.date_range_custom.0);
                    });
                }
                egui::ComboBox::from_label("To date")
                    .selected_text(self.date_range_option.1.to_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.date_range_option.1,
                            DateRangeOption::Today,
                            DateRangeOption::Today.to_str(),
                        );
                        ui.selectable_value(
                            &mut self.date_range_option.1,
                            DateRangeOption::Custom,
                            DateRangeOption::Custom.to_str(),
                        );
                    });
                if self.date_range_option.1 == DateRangeOption::Custom {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.date_range_custom.1);
                    });
                }

                egui::ComboBox::from_label("Filter by branch")
                    .selected_text(self.branch.as_deref().unwrap_or("All branches"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.branch, None, "All branches");
                        ui.selectable_value(&mut self.branch, Some("OH".to_string()), "OH");
                        ui.selectable_value(&mut self.branch, Some("MI".to_string()), "MI");
                    });

                egui::ComboBox::from_label("Filter by lead source")
                    .selected_text(self.lead_source_option.as_deref().unwrap_or("All sources"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.lead_source_option, None, "All sources");
                        for lead_source in &jn_data.lead_sources {
                            ui.selectable_value(
                                &mut self.lead_source_option,
                                Some(lead_source.clone()),
                                lead_source,
                            );
                        }
                    });

                if ui.button("Calculate KPIs").clicked() {
                    let jn_data = Arc::clone(jn_data);
                    self.start_calculate(jn_data);
                }
            } else {
                ui.label("No JobNimbus data available; use the button to fetch");
                return;
            }
        });

        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("KPI Stats Viewer");
            if let Some(kpi_data) = self.kpi_data.get_mut().as_ref() {
                render_stats_viewer(ui, &mut self.selected_rep, kpi_data);
            } else {
                ui.label("No KPI data available; calculate KPIs first");
            }
        });

        egui::Frame::group(&egui::Style::default()).show(ui, |ui| {
            ui.heading("Export to Google Sheets");
            ui.horizontal(|ui| {
                ui.label("Spreadsheet ID (empty to create):");
                let spreadsheet_id = self.spreadsheet_id.get_mut();
                if let Some(new_spreadsheet_id) = self.export_data.get_mut().take() {
                    *spreadsheet_id = new_spreadsheet_id;
                }
                ui.text_edit_singleline(spreadsheet_id);
            });
            ui.horizontal(|ui| {
                let kpi_data = self.kpi_data.get_mut();
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
                        Some(self.spreadsheet_id.get().clone()).filter(|s| !s.is_empty());
                    if let Some(data) = kpi_data.as_ref().map(|a| Arc::clone(a)) {
                        // stop borrowing self before we borrow it again to
                        // generate the google sheets
                        drop(kpi_data);
                        self.start_generate_google_sheets(data, spreadsheet_id);
                    }
                }
            });
        });
    }

    fn start_calculate(&mut self, jn_data: Arc<JobNimbusData>) {
        let date_range = match self.get_date_range() {
            Ok(date_range) => date_range,
            Err(e) => {
                warn!("error parsing date range: {}", e);
                return;
            }
        };
        let kpi_data_tx = self.kpi_data.start_fetch();
        let lead_source_filter = self.lead_source_option.clone();
        let branch_filter = self.branch.clone();
        thread::spawn(move || {
            let jobs = jn_data
                .jobs
                .iter()
                .cloned()
                .filter(|job| lead_source_filter.is_none() || job.lead_source == lead_source_filter)
                .filter(|job| branch_filter.is_none() || job.state == branch_filter);
            let unsettled_date = chrono::Local::now().to_utc();
            let abandon_date = chrono::Local::now().to_utc() - chrono::Duration::days(60);
            let kpi_data =
                tools::kpi::calculate_kpi(jobs, date_range, unsettled_date, abandon_date);
            let _ = kpi_data_tx.send(Some(Arc::new(kpi_data)));
        });
    }

    fn get_date_range(&self) -> anyhow::Result<DateRange> {
        let from_text = match self.date_range_option.0 {
            DateRangeOption::Custom => &self.date_range_custom.0,
            preset => preset.to_str(),
        };
        let to_text = match self.date_range_option.1 {
            DateRangeOption::Custom => &self.date_range_custom.1,
            preset => preset.to_str(),
        };
        DateRange::from_strs(from_text, to_text)
    }

    fn start_generate_google_sheets(
        &mut self,
        kpi_data: Arc<KpiData>,
        spreadsheet_id: Option<String>,
    ) {
        let export_complete_tx = self.export_data.start_fetch();
        thread::spawn(move || {
            let new_spreadsheet_id = tools::kpi::output::generate_report_google_sheets(
                &kpi_data,
                spreadsheet_id.as_deref(),
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

fn render_stats_viewer(
    ui: &mut egui::Ui,
    selected_rep: &mut Option<KpiSubject>,
    kpi_data: &KpiData,
) {
    // display and allow user to choose current tracker
    let heading =
        ui.button(selected_rep.as_ref().map_or("No rep selected (click me)", |rep| rep.as_str()));
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

#[derive(Default, Copy, Clone, PartialEq)]
enum DateRangeOption {
    #[default]
    Forever,
    StartOfYear,
    Today,
    Custom,
}

impl DateRangeOption {
    const fn to_str(self) -> &'static str {
        match self {
            Self::Forever => "Forever",
            Self::StartOfYear => "Start-of-year",
            Self::Today => "Today",
            Self::Custom => "Custom",
        }
    }
}
