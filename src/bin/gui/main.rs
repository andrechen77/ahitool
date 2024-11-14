
use eframe::egui;
use job_nimbus_client::JobNimbusClient;
use tracing::warn;

mod data_loader;
mod job_nimbus_client;
mod kpi_page;

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
    job_nimbus_client: JobNimbusClient,
    kpi_page_state: kpi_page::KpiPage,
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
                AhitoolTool::Kpi => self.kpi_page_state.render(ui, &mut self.job_nimbus_client),
                AhitoolTool::Ar => {}
            }
        });
    }
}
