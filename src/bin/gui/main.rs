use ahitool::utils::FileBacked;
use eframe::egui;
use job_nimbus_client::JobNimbusClient;
use job_viewer::JobNimbusViewer;
use tracing::warn;

mod ar_page;
mod data_loader;
mod job_nimbus_client;
mod job_viewer;
mod kpi_page;

fn main() {
    // set up tracing
    tracing_subscriber::fmt::init();

    let app_state = AppState::with_cached_storage();

    // run the UI on the main thread
    let result =
        eframe::run_native("AHItool", Default::default(), Box::new(|_cc| Ok(Box::new(app_state))));
    if let Err(e) = result {
        warn!("error in UI thread: {}", e);
    }
}

struct AppState {
    current_tool: AhitoolTool,
    job_nimbus_client: JobNimbusClient,
    job_nimbus_viewer: JobNimbusViewer,
    kpi_page_state: kpi_page::KpiPage,
    ar_page_state: ar_page::ArPage,
}

#[derive(Default, PartialEq, Eq, Hash)]
enum AhitoolTool {
    #[default]
    None,
    JobViewer,
    Kpi,
    Ar,
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // heading to display and choose the current tool
            let heading = ui.heading(match self.current_tool {
                AhitoolTool::None => "Welcome to AHItool",
                AhitoolTool::JobViewer => "Job Viewer",
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
                    if ui.button("Job Viewer").clicked() {
                        self.current_tool = AhitoolTool::JobViewer;
                    }
                },
            );

            // display the current tool
            match self.current_tool {
                AhitoolTool::None => {
                    ui.label("Click on the heading to choose a subtool.");
                }
                AhitoolTool::JobViewer => {
                    self.job_nimbus_viewer.render(ui, &mut self.job_nimbus_client)
                }
                AhitoolTool::Kpi => self.kpi_page_state.render(ui, &mut self.job_nimbus_client),
                AhitoolTool::Ar => self.ar_page_state.render(ui, &mut self.job_nimbus_client),
            }
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.job_nimbus_client.on_exit();
        self.kpi_page_state.on_exit();
        self.ar_page_state.on_exit();
    }
}

impl AppState {
    fn with_cached_storage() -> Self {
        Self {
            kpi_page_state: kpi_page::KpiPage::new(FileBacked::new_from_file_or(
                "kpi_spreadsheet_id.json",
                || String::new(),
            )),
            ar_page_state: ar_page::ArPage::new(FileBacked::new_from_file_or(
                "ar_spreadsheet_id.json",
                || String::new(),
            )),
            job_nimbus_client: JobNimbusClient::new(FileBacked::new_from_file_or(
                "job_nimbus_api_key.json",
                || String::new(),
            )),
            job_nimbus_viewer: JobNimbusViewer::new(),
            current_tool: AhitoolTool::None,
        }
    }
}
