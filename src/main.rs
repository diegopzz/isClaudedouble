fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--popup") {
        isclaude2x_tray::popup::run_popup()
    } else {
        isclaude2x_tray::runtime::run()
    }
}
