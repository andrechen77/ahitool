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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_icon(icon_data()),
        ..Default::default()
    };

    // run the UI on the main thread
    let result = eframe::run_native("AHItool", options, Box::new(|_cc| Ok(Box::new(app_state))));
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

fn icon_data() -> egui::IconData {
    /// FUTURE extract this automatically from the image
    const WIDTH: u32 = 256;
    const HEIGHT: u32 = 256;
    const DATA: &[u8] = include_bytes!("icon.rgba8");

    egui::IconData { width: WIDTH as _, height: HEIGHT as _, rgba: DATA.to_vec() }
}
