use ahitool::utils::FileBacked;

use crate::{
    ar_page::ArPage, job_nimbus_client::JobNimbusClient, job_viewer::JobNimbusViewer,
    kpi_page::KpiPage,
};

pub struct MainApp {
    pub current_tool: AhitoolTool,
    pub job_nimbus_client: JobNimbusClient,
    pub job_nimbus_viewer: JobNimbusViewer,
    pub kpi_page_state: KpiPage,
    pub ar_page_state: ArPage,
}

#[derive(Default, PartialEq, Eq, Hash)]
pub enum AhitoolTool {
    #[default]
    None,
    JobViewer,
    Kpi,
    Ar,
}

impl MainApp {
    pub fn with_cached_storage() -> Self {
        Self {
            kpi_page_state: KpiPage::new(FileBacked::new_from_file_or(
                "kpi_spreadsheet_id.json",
                || String::new(),
            )),
            ar_page_state: ArPage::new(FileBacked::new_from_file_or(
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

    pub fn render(&mut self, ui: &mut egui::Ui) {
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
    }

    pub fn on_exit(&mut self) {
        self.job_nimbus_client.on_exit();
        self.kpi_page_state.on_exit();
        self.ar_page_state.on_exit();
    }
}
