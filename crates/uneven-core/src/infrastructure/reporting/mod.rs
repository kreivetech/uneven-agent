pub mod html;
pub mod excel;
pub mod packager;
pub mod logger;

pub use html::{generate_html_report, generate_dashboard, generate_multi_dashboard};
pub use excel::ExcelReport;
pub use packager::package_to_exe;
pub use logger::MarkdownLogger;
