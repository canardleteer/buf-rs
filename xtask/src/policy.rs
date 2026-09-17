//! Repository-owned seams for check order and coverage report paths.

use crate::CheckStep;

pub const CHECK_ORDER: [CheckStep; 4] = [
    CheckStep::Fmt,
    CheckStep::Check,
    CheckStep::Clippy,
    CheckStep::Test,
];

pub const LLVM_COV_REPORT: &str = "target/coverage/llvm-cov/html/index.html";
pub const TARPAULIN_REPORT: &str = "target/coverage/tarpaulin/tarpaulin-report.html";
