// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(debug_assertions)]
    {
        let mut args = std::env::args().skip(1);
        match args.next().as_deref() {
            Some("--probe") => return collector_lib::probe(args.collect()),
            Some("--probe-follows") => return collector_lib::probe_follows(),
            _ => {}
        }
    }
    collector_lib::run()
}
