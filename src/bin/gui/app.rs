use std::path::PathBuf;

use ahitool::utils::FileBacked;
use tracing::warn;

use crate::{
    all_jobs_page::AllJobsPage, ar_page::ArPage, debug_print::DebugPrint,
    job_nimbus_client::JobNimbusClient, kpi_page::KpiPage, update_page::UpdatePage,
};

pub struct MainApp {
    pub current_tool: AhitoolTool,
    pub job_nimbus_client: JobNimbusClient,
    pub oauth_cache_file: PathBuf,
    pub debug_print: DebugPrint,
    pub kpi_page_state: KpiPage,
    pub all_jobs_page_state: AllJobsPage,
    pub ar_page_state: ArPage,
    pub update_page_state: UpdatePage,
}

#[derive(Default, PartialEq, Eq, Hash)]
pub enum AhitoolTool {
    #[default]
    None,
    DebugPrint,
    Kpi,
    AllJobs,
    Ar,
    SelfUpdate,
}

impl MainApp {
    pub fn with_cached_storage() -> Self {
        let config_dir = dirs::config_dir().unwrap().join("ahitool");
        if let Err(e) = std::fs::create_dir_all(&config_dir) {
            warn!("Failed to create config directory {:?}: {}", config_dir, e);
        }

        let mut new = Self {
            kpi_page_state: KpiPage::new(FileBacked::new_from_file_or(
                config_dir.join("kpi_spreadsheet_id.json"),
                || String::new(),
            )),
            all_jobs_page_state: AllJobsPage::new(FileBacked::new_from_file_or(
                config_dir.join("all_jobs_spreadsheet_id.json"),
                || String::new(),
            )),
            ar_page_state: ArPage::new(FileBacked::new_from_file_or(
                config_dir.join("ar_spreadsheet_id.json"),
                || String::new(),
            )),
            job_nimbus_client: JobNimbusClient::new(FileBacked::new_from_file_or(
                config_dir.join("job_nimbus_api_key.json"),
                || String::new(),
            )),
            debug_print: DebugPrint::new(),
            update_page_state: UpdatePage::new(),
            current_tool: AhitoolTool::None,
            oauth_cache_file: config_dir.join("google_oauth_token.json"),
        };
        new.job_nimbus_client.start_fetch();
        new
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        // heading to display and choose the current tool
        let popup_id = ui.make_persistent_id("tool_chooser");
        let heading = ui.horizontal(|ui| {
            ui.heading(match self.current_tool {
                AhitoolTool::None => "Welcome to AHItool",
                AhitoolTool::DebugPrint => "Job Viewer",
                AhitoolTool::Kpi => "Key Performance Indicators",
                AhitoolTool::AllJobs => "All Jobs",
                AhitoolTool::Ar => "Accounts Receivable",
                AhitoolTool::SelfUpdate => "Self Update",
            });
            if ui.button("Change tool").clicked() {
                ui.memory_mut(|mem| mem.toggle_popup(popup_id));
            }
        });
        let heading = heading.response;
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
                if ui.button("All Jobs").clicked() {
                    self.current_tool = AhitoolTool::AllJobs;
                }
                if ui.button("AR").clicked() {
                    self.current_tool = AhitoolTool::Ar;
                }
                if ui.button("Debug Print").clicked() {
                    self.current_tool = AhitoolTool::DebugPrint;
                }
                if ui.button("Self Update").clicked() {
                    self.current_tool = AhitoolTool::SelfUpdate;
                }
            },
        );

        // display the current tool
        match self.current_tool {
            AhitoolTool::None => {
                ui.label("Please choose a subtool.");
            }
            AhitoolTool::DebugPrint => self.debug_print.render(ui, &mut self.job_nimbus_client),
            AhitoolTool::Kpi => {
                self.kpi_page_state.render(ui, &mut self.job_nimbus_client, &self.oauth_cache_file)
            }
            AhitoolTool::AllJobs => self.all_jobs_page_state.render(
                ui,
                &mut self.job_nimbus_client,
                &self.oauth_cache_file,
            ),
            AhitoolTool::Ar => {
                self.ar_page_state.render(ui, &mut self.job_nimbus_client, &self.oauth_cache_file)
            }
            AhitoolTool::SelfUpdate => self.update_page_state.render(ui),
        }
    }

    pub fn on_exit(&mut self) {
        self.job_nimbus_client.on_exit();
        self.kpi_page_state.on_exit();
        self.ar_page_state.on_exit();
        self.all_jobs_page_state.on_exit();
    }
}
