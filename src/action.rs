use serde::{Deserialize, Serialize};

/// All user intents (actions) that can be performed in the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Quit the application
    Quit,

    /// Toggle auto-update
    ToggleAutoUpdate,

    /// Toggle graphs display
    ToggleGraphs,

    /// Increase update speed
    IncreaseSpeed,

    /// Decrease update speed
    DecreaseSpeed,

    /// Switch to next tab
    NextTab,

    /// Move selection up
    MoveUp,

    /// Move selection down
    MoveDown,

    /// Enter detailed view for selected item
    Enter,

    /// Go back from detailed view
    Back,

    /// Switch to next color scheme
    SwitchTheme,

    /// Switch to next process sorting option
    SwitchProcessSort,

    /// Switch to next chart type
    SwitchChartType,

    /// Toggle process tree view
    ToggleProcessTree,

    /// Start process search/filter input
    StartProcessSearch,

    /// Append one character to process search input
    UpdateProcessSearch(char),

    /// Delete one character from process search input
    BackspaceProcessSearch,

    /// Confirm process search input
    ConfirmProcessSearch,

    /// Cancel process search input
    CancelProcessSearch,

    /// Open process termination confirmation for selected process
    RequestProcessKill,

    /// Toggle selection between Yes/No in kill confirmation
    ToggleProcessKillChoice,

    /// Confirm process termination selection
    ConfirmProcessKill,

    /// Cancel process termination confirmation
    CancelProcessKill,

    /// Cycle through network interfaces
    CycleNetworkInterface,

    /// Save current configuration
    SaveConfig,

    /// No operation
    Tick,

    /// Error occurred
    Error(String),
}
