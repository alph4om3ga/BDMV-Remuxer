use std::io;

pub fn format_tool_launch_error(
    tool: &str,
    path: &str,
    settings_field: &str,
    install_hint: &str,
    error: io::Error,
) -> String {
    format!(
        "Could not start {tool} at \"{path}\": {error}. Install {install_hint} and set {settings_field} in Settings to the executable path."
    )
}
