use std::error::Error;
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    let buf = buf_tools::buf_bin_path();
    let status = Command::new(buf)
        .arg("lint")
        .arg("proto") // Directory containing your buf.yaml and .proto files
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("buf lint failed: {status}").into())
    }
}
