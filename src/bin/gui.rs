use ahitool::{
    apis::job_nimbus,
    tools::{self, kpi::{JobTrackerStats, KpiData, KpiSubject}},
};
use chrono::{DateTime, Utc};
use eframe::egui;
use tokio::sync::watch;
use tracing::warn;

mod resource {
    use std::sync::OnceLock;
    use tokio::runtime;

    pub fn runtime() -> &'static runtime::Runtime {
        static RUNTIME: OnceLock<runtime::Runtime> = OnceLock::new();
        let rt = RUNTIME
            .get_or_init(|| runtime::Builder::new_multi_thread().enable_all().build().unwrap());
        rt
    }

    pub fn client() -> reqwest::Client {
        static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
        let client = CLIENT.get_or_init(|| reqwest::Client::new());
        client.clone()
    }
}

fn main() {
    // set up tracing
    tracing_subscriber::fmt::init();

    // make sure the runtime is initialized
    resource::runtime();

    // run the UI on the main thread
    let result = eframe::run_native(
        "AHItool",
        Default::default(),
        Box::new(|_cc| Ok(Box::new(AhitoolApp::default()))),
    );
    if let Err(e) = result {
        warn!("error in UI thread: {}", e);
    }
}

#[derive(Default)]
struct AhitoolApp {
    current_tool: AhitoolTool,
    kpi_page_state: KpiPage,
}

#[derive(Default, PartialEq, Eq, Hash)]
enum AhitoolTool {
    #[default]
    None,
    Kpi,
    Ar,
}

impl eframe::App for AhitoolApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // heading to display and choose the current tool
            let heading = ui.heading(match self.current_tool {
                AhitoolTool::None => "Welcome to AHItool",
                AhitoolTool::Kpi => "Key Performance Indicators",
                AhitoolTool::Ar => "Accounts Receivable",
            });
            let popup_id = ui.make_persistent_id("tool_chooser");
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
                    ui.label("Select a tool:");
                    if ui.button("KPI").clicked() {
                        self.current_tool = AhitoolTool::Kpi;
                    }
                    if ui.button("AR").clicked() {
                        self.current_tool = AhitoolTool::Ar;
                    }
                },
            );

            // display the current tool
            match self.current_tool {
                AhitoolTool::None => {}
                AhitoolTool::Kpi => render_kpi_page(ui, &mut self.kpi_page_state),
                AhitoolTool::Ar => {}
            }
        });
    }
}

struct KpiPage {
    show_api_key: bool,
    jn_api_key: String,
    loading_kpi_result: bool,
    kpi_result_tx: watch::Sender<Option<KpiResult>>,
    kpi_result_rx: watch::Receiver<Option<KpiResult>>,
    selected_rep: Option<KpiSubject>,
}

impl Default for KpiPage {
    fn default() -> Self {
        let (tx, rx) = watch::channel(None);
        KpiPage {
            show_api_key: false,
            jn_api_key: String::new(),
            loading_kpi_result: false,
            kpi_result_tx: tx,
            kpi_result_rx: rx,
            selected_rep: None,
        }
    }
}

struct KpiResult {
    last_fetched: DateTime<Utc>,
    data: KpiData,
}

async fn fetch_kpi_result(jn_api_key: String, answerer: watch::Sender<Option<KpiResult>>) {
    let answer = match job_nimbus::get_all_jobs_from_job_nimbus(resource::client(), &jn_api_key, None).await {
        Ok(jobs) => {
            let last_fetched = Utc::now();
            let kpi_result = tools::kpi::calculate_kpi(jobs, (None, None));
            Some(KpiResult { last_fetched, data: kpi_result })
        }
        Err(e) => {
            warn!("error fetching jobs: {}", e);
            None
        }
    };
    if let Err(e) = answerer.send(answer) {
        warn!("internal communication error: {}", e);
    }
}

fn render_kpi_page(ui: &mut egui::Ui, state: &mut KpiPage) {
    ui.horizontal(|ui| {
        ui.label("JobNimbus API Key:");
        ui.checkbox(&mut state.show_api_key, "show");
        if state.show_api_key {
            ui.text_edit_singleline(&mut state.jn_api_key);
        } else {
            ui.text_edit_singleline(&mut "************");
        }
    });

    if let Ok(true) = state.kpi_result_rx.has_changed() {
        state.loading_kpi_result = false;
    }
    let kpi_result = state.kpi_result_rx.borrow_and_update();

    ui.horizontal(|ui| {
        if state.loading_kpi_result {
            ui.label("Loading...");
        } else if ui.button("Fetch Jobs").clicked() {
            state.loading_kpi_result = true;
            resource::runtime()
                .spawn(fetch_kpi_result(state.jn_api_key.clone(), state.kpi_result_tx.clone()));
        }
        ui.label(format!(
            "Last fetched: {}",
            kpi_result
                .as_ref()
                .map(|d| d.last_fetched.time().to_string())
                .as_deref()
                .unwrap_or("never")
        ));
    });
    ui.separator();
    if let Some(kpi_result) = kpi_result.as_ref() {
        // display and allow user to choose current tracker
        let heading = ui.label(state.selected_rep.as_ref().map_or("no rep selected", |rep| rep.as_str()));
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
                ui.label("Select a tool:");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (rep, _) in &kpi_result.data.stats_by_rep {
                        if ui.button(rep.as_str()).clicked() {
                            state.selected_rep = Some(rep.clone());
                        }
                    }
                });
            },
        );

        if let Some(selected_rep) = state.selected_rep.as_ref() {
            if let Some(stats) = kpi_result.data.stats_by_rep.get(selected_rep) {
                render_kpi_stats_table(ui, stats);
            } else {
                ui.label("No stats available for selected rep");
            }
        }
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
