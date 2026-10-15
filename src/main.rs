// Release builds use the GUI subsystem, so no console window opens next to the calculator.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod engine;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Calculator")
            .with_inner_size([320.0, 500.0])
            .with_min_inner_size([280.0, 360.0])
            .with_icon(app::window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Calculator",
        options,
        Box::new(|cc| Ok(Box::new(app::CalculatorApp::new(cc)))),
    )
}
