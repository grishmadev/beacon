pub mod app;
pub mod ui;

pub fn sigrate_to_bars(sigrate: i32) -> String {
    let sigrate = -sigrate;
    let bar = if sigrate < 30 {
        "BEST"
    } else if (30..=50).contains(&sigrate) {
        "||||||||"
    } else if (50..=60).contains(&sigrate) {
        "||||||"
    } else if (60..=67).contains(&sigrate) {
        "||||"
    } else if (67..=80).contains(&sigrate) {
        "|||"
    } else if (80..=90).contains(&sigrate) {
        "||"
    } else {
        "---"
    };
    bar.to_string()
}
