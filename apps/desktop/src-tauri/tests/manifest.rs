//! The program's Windows manifest, as built into it (SEC-4, threat T16).

/// The app runs as the user who started it and never asks for
/// administrator rights. It keeps Common Controls 6, which the folder
/// dialogs need.
#[cfg(windows)]
#[test]
fn the_app_runs_as_the_user_and_never_asks_for_elevation() {
    let program = std::fs::read(env!("CARGO_BIN_EXE_catchword-desktop")).unwrap();
    let holds = |text: &str| program.windows(text.len()).any(|w| w == text.as_bytes());
    assert!(holds(r#"requestedExecutionLevel level="asInvoker""#));
    assert!(!holds("requireAdministrator"));
    assert!(!holds("highestAvailable"));
    assert!(holds("Microsoft.Windows.Common-Controls"));
}
