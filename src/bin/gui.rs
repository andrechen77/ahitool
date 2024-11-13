use ahitool::{apis::job_nimbus, tools};
use chrono::{DateTime, Utc};
use eframe::egui;
use tracing::warn;

fn main() {
    // set up tracing
    tracing_subscriber::fmt::init();

    eframe::run_native(
        "AHItool",
        Default::default(),
        Box::new(|_cc| Ok(Box::new(AhitoolApp::default()))),
    )
    .unwrap();
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
        // Draw the menu bar at the top
        // egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        //     egui::menu::bar(ui, |ui| {
        //         ui.menu_button("Tools", |ui| {
        //             if ui.button("KPI").clicked() {
        //                 self.current_tool = AhitoolTool::Kpi;
        //                 ui.close_menu();
        //             }
        //             if ui.button("AR").clicked() {
        //                 self.current_tool = AhitoolTool::Ar;
        //                 ui.close_menu();
        //             }
        //         });
        //     });
        // });

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
                AhitoolTool::Kpi => kpi_page(ui, &mut self.kpi_page_state),
                AhitoolTool::Ar => {}
            }
        });
    }
}

#[derive(Default)]
struct KpiPage {
    show_api_key: bool,
    jn_api_key: String,
    last_fetched: Option<DateTime<Utc>>,
    report: String,
}

fn kpi_page(ui: &mut egui::Ui, state: &mut KpiPage) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_api_key, "Show API Key");
        if state.show_api_key {
            ui.text_edit_singleline(&mut state.jn_api_key);
        } else {
            ui.label("********");
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Fetch Jobs").clicked() {
            match job_nimbus::get_all_jobs_from_job_nimbus(&state.jn_api_key, None) {
                Ok(jobs) => {
                    state.last_fetched = Some(Utc::now());
                    let kpi_result = tools::kpi::calculate_kpi(jobs, (None, None));

                    let mut output = Vec::new();
                    match tools::kpi::output::human::print_entire_report_to_writer(
                        &kpi_result,
                        &mut output,
                    ) {
                        Ok(_) => {
                            state.report =
                                String::from_utf8(output).expect("output should be valid UTF-8");
                        }
                        Err(e) => {
                            warn!("error formatting KPI report: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("error fetching jobs: {}", e);
                }
            }
        }
        ui.label(format!(
            "Last fetched: {}",
            state.last_fetched.map(|d| d.time().to_string()).as_deref().unwrap_or("never")
        ));
    });
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.label(&state.report);
    });
}
