use std::thread;

use ahitool::tools::update;
use tracing::{info, warn};

use crate::data_loader::DataLoader;

#[derive(Default)]
pub struct UpdatePage {
    updating: DataLoader<()>,
}

impl UpdatePage {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        let in_progress = self.updating.fetch_in_progress();
        let button = ui.add_enabled(!in_progress, egui::Button::new("Download and install update"));
        if in_progress {
            ui.label("Updating...");
        } else {
            if button.clicked() {
                self.start_update();
            }
        }
    }

    fn start_update(&mut self) {
        info!("Starting self-update");
        let completion_tx = self.updating.start_fetch();
        thread::spawn(move || {
            if let Err(e) = update::update_executable(update::GITHUB_REPO) {
                warn!("Error while updating executable: {}", e);
            } else {
                info!("Successfully updated executable");
            }
            let _ = completion_tx.send(());
        });
    }
}
