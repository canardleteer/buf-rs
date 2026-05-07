use std::error::Error;
use std::process::Command;

/// Baseline Buf image for `protoc-gen-buf-breaking` (`proto/breaking_against.binpb`).
/// Generate it under this crate's `proto/` directory before running (see README).
const BUF_BREAKING_OPT: &str =
    r#"{"against_input":"proto/breaking_against.binpb","limit_to_input_files":true}"#;

fn main() -> Result<(), Box<dyn Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let lint_plugin = buf_sys::protoc_gen_buf_lint_bin_path();
    let breaking_plugin = buf_sys::protoc_gen_buf_breaking_bin_path();

    let status = Command::new(protoc)
        .arg("--proto_path=proto")
        .arg("--plugin=protoc-gen-buf-lint")
        .arg(format!(
            "--plugin=protoc-gen-buf-lint={}",
            lint_plugin.display()
        ))
        .arg("--plugin=protoc-gen-buf-breaking")
        .arg(format!(
            "--plugin=protoc-gen-buf-breaking={}",
            breaking_plugin.display()
        ))
        .arg("--buf-lint_out=.")
        .arg("--buf-breaking_out=.")
        .arg(format!("--buf-breaking_opt={BUF_BREAKING_OPT}"))
        .arg("proto/acme/weather/v1/weather.proto")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("protoc plugin run failed: {status}").into())
    }
}
