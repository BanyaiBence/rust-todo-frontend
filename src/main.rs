use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use todo_frontend::app::App;

fn main() {
    let window = WindowBuilder::new()
        .with_title("Todos")
        .with_inner_size(LogicalSize::new(560.0, 760.0))
        .with_min_inner_size(LogicalSize::new(380.0, 480.0));

    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window).with_menu(None))
        .launch(App);
}
