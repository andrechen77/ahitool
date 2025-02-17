use app::MainApp;
use eframe::egui;
use tracing::warn;

mod app;
mod ar_page;
mod data_loader;
mod job_nimbus_client;
mod job_viewer;
mod kpi_page;
mod update_page;

fn main() {
    // set up tracing
    tracing_subscriber::fmt::init();

    let app_state = MainApp::with_cached_storage();

    // run the UI on the main thread
    let result =
        eframe::run_native("AHItool", Default::default(), Box::new(|_cc| Ok(Box::new(app_state))));
    if let Err(e) = result {
        warn!("Error in UI thread: {}", e);
    }
}

impl eframe::App for MainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render(ui);
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.on_exit();
    }
}
