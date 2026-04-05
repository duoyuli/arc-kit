use console::style;
use dialoguer::theme::ColorfulTheme;

pub(crate) fn theme() -> ColorfulTheme {
    ColorfulTheme {
        checked_item_prefix: style("☑".to_string()).for_stderr().green(),
        unchecked_item_prefix: style("☐".to_string()).for_stderr().dim(),
        ..ColorfulTheme::default()
    }
}
