pub(crate) mod migrate;
mod model;
mod paths;
mod store;

pub use model::{
    Accent, AppearanceSettings, Density, GameSettings, Integration, LaunchOutcome, LaunchRecord,
    QuickTarget, Settings, StartupTarget, State, ThemeMode, WindowState,
};
pub use paths::Paths;
pub use store::Store;
