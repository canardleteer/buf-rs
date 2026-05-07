//! Optional upstream source archive (GitHub tag tarball).

use std::fs::File;
use std::path::Path;

use flate2::read::GzDecoder;
use tar::Archive;

/// Extract `buf-{tag}.tar.gz` style archive into `dest` (directory).
pub fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
    let file =
        File::open(archive_path).map_err(|e| format!("open {}: {e}", archive_path.display()))?;
    let dec = GzDecoder::new(file);
    let mut tar = Archive::new(dec);
    tar.unpack(dest).map_err(|e| {
        format!(
            "unpack {} -> {}: {e}",
            archive_path.display(),
            dest.display()
        )
    })?;
    Ok(())
}
