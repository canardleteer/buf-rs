//! Local OCI image builds with Docker and Buildah (Debian + Alpine integration).

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::policy;
use crate::process::{CommandSpec, best_effort, output, probe, run as run_command};
use crate::publish::expected_buf_core;
use crate::{ImageEngine, ImageTarget};

const DOCKER_GUIDANCE: &str =
    "Follow https://docs.docker.com/engine/install/ and verify the local daemon is reachable.";
const BUILDAH_GUIDANCE: &str = "Follow https://github.com/containers/buildah/blob/main/install.md and verify its storage/runtime setup.";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImagePlan {
    file: &'static str,
    flavor: &'static str,
    tag: String,
}

pub(crate) fn run(root: &Path, requested: ImageEngine, target: ImageTarget) -> Result<()> {
    let engine = resolve_engine(root, requested)?;
    let context = stage_path_context(root)?;
    let expect_core = expected_buf_core(&root.join("Cargo.toml"))?;
    for plan in plans(target) {
        match engine {
            ImageEngine::Docker => docker(root, &context, &plan, &expect_core)?,
            ImageEngine::Buildah => buildah(root, &context, &plan, &expect_core)?,
            ImageEngine::Auto => unreachable!("auto is resolved before execution"),
        }
    }
    Ok(())
}

fn plans(target: ImageTarget) -> Vec<ImagePlan> {
    match target {
        ImageTarget::Debian(args) => vec![ImagePlan {
            file: policy::IMAGE_DEBIAN_FILE,
            flavor: policy::IMAGE_DEBIAN_FLAVOR,
            tag: args.tag,
        }],
        ImageTarget::Alpine(args) => vec![ImagePlan {
            file: policy::IMAGE_ALPINE_FILE,
            flavor: policy::IMAGE_ALPINE_FLAVOR,
            tag: args.tag,
        }],
        ImageTarget::All(args) => vec![
            ImagePlan {
                file: policy::IMAGE_DEBIAN_FILE,
                flavor: policy::IMAGE_DEBIAN_FLAVOR,
                tag: args.debian_tag,
            },
            ImagePlan {
                file: policy::IMAGE_ALPINE_FILE,
                flavor: policy::IMAGE_ALPINE_FLAVOR,
                tag: args.alpine_tag,
            },
        ],
    }
}

fn rust_docker_tag(root: &Path, flavor: &str) -> Result<String> {
    output(
        root,
        &CommandSpec::new("bash").args([policy::IMAGE_TAG_SCRIPT, "rust-toolchain.toml", flavor]),
    )
}

fn resolve_engine(root: &Path, requested: ImageEngine) -> Result<ImageEngine> {
    if requested != ImageEngine::Auto {
        engine_probe(root, requested).map_err(anyhow::Error::msg)?;
        return Ok(requested);
    }

    let mut failures = Vec::new();
    for engine in policy::IMAGE_AUTO_ORDER {
        match engine_probe(root, engine) {
            Ok(()) => return Ok(engine),
            Err(error) => {
                eprintln!("Skipping {engine:?}: {error}");
                failures.push(error);
            }
        }
    }
    bail!(
        "no usable OCI engine in configured auto order:\n- {}",
        failures.join("\n- ")
    )
}

fn engine_probe(root: &Path, engine: ImageEngine) -> Result<(), String> {
    match engine {
        ImageEngine::Docker => probe(
            root,
            "Docker",
            &CommandSpec::new("docker").args(["info", "--format", "{{.ServerVersion}}"]),
            DOCKER_GUIDANCE,
        ),
        ImageEngine::Buildah => probe(
            root,
            "Buildah",
            &CommandSpec::new("buildah").arg("info"),
            BUILDAH_GUIDANCE,
        ),
        ImageEngine::Auto => Err("auto is not an executable engine".to_string()),
    }
}

fn docker(root: &Path, context: &Path, plan: &ImagePlan, expect_core: &str) -> Result<()> {
    let rust_tag = rust_docker_tag(root, plan.flavor)?;
    let mut build = CommandSpec::new("docker").arg("build");
    append_build_args(
        &mut build.args,
        &[
            ("RUST_DOCKER_TAG".to_string(), rust_tag),
            ("INSTALL_MODE".to_string(), "path".to_string()),
        ],
    );
    build.args.extend([
        "--tag".into(),
        plan.tag.clone().into(),
        "--file".into(),
        context.join(plan.file).into(),
        context.as_os_str().to_os_string(),
    ]);
    run_command(root, &build)?;

    run_command(
        root,
        &CommandSpec::new("docker").args([
            "run",
            "--rm",
            "-e",
            &format!("EXPECT_BUF_CORE={expect_core}"),
            plan.tag.as_str(),
        ]),
    )
}

fn buildah(root: &Path, context: &Path, plan: &ImagePlan, expect_core: &str) -> Result<()> {
    let rust_tag = rust_docker_tag(root, plan.flavor)?;
    let mut build = CommandSpec::new("buildah").arg("bud");
    append_build_args(
        &mut build.args,
        &[
            ("RUST_DOCKER_TAG".to_string(), rust_tag),
            ("INSTALL_MODE".to_string(), "path".to_string()),
        ],
    );
    build.args.extend([
        "--tag".into(),
        plan.tag.clone().into(),
        "--file".into(),
        context.join(plan.file).into(),
        context.as_os_str().to_os_string(),
    ]);
    run_command(root, &build)?;

    let name = temporary_name();
    let result = (|| {
        run_command(
            root,
            &CommandSpec::new("buildah").args(["from", "--name", name.as_str(), plan.tag.as_str()]),
        )?;
        run_command(
            root,
            &CommandSpec::new("buildah").args([
                "run",
                "--env",
                &format!("EXPECT_BUF_CORE={expect_core}"),
                name.as_str(),
                "--",
                policy::IMAGE_SMOKE_PROGRAM,
            ]),
        )
    })();
    best_effort(
        root,
        &CommandSpec::new("buildah").args(["rm", name.as_str()]),
    );
    result
}

fn append_build_args(args: &mut Vec<OsString>, build_args: &[(String, String)]) {
    for (name, value) in build_args {
        args.push("--build-arg".into());
        args.push(format!("{name}={value}").into());
    }
}

fn temporary_name() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    format!("xtask-smoke-{}-{millis}", std::process::id())
}

fn stage_path_context(root: &Path) -> Result<PathBuf> {
    let ctx = root.join(policy::IMAGE_CONTEXT_DIR);
    if ctx.exists() {
        fs::remove_dir_all(&ctx).with_context(|| format!("remove {}", ctx.display()))?;
    }
    fs::create_dir_all(ctx.join("proto")).with_context(|| format!("create {}", ctx.display()))?;
    fs::create_dir_all(ctx.join("path-src"))?;

    copy_file(
        root.join("rust-toolchain.toml"),
        ctx.join("rust-toolchain.toml"),
    )?;
    copy_file(
        root.join(".github/ci/integration/Cargo.toml"),
        ctx.join("Cargo.toml"),
    )?;
    copy_file(
        root.join(".github/ci/integration/Dockerfile"),
        ctx.join("Dockerfile"),
    )?;
    copy_file(
        root.join(".github/ci/integration/Dockerfile.alpine"),
        ctx.join("Dockerfile.alpine"),
    )?;
    copy_file(
        root.join(".github/ci/integration/entrypoint.sh"),
        ctx.join("entrypoint.sh"),
    )?;
    copy_file(root.join("examples/buf_lint.rs"), ctx.join("buf_lint.rs"))?;
    copy_file(
        root.join("examples/protoc_with_buf_plugins.rs"),
        ctx.join("protoc_with_buf_plugins.rs"),
    )?;
    run_command(
        root,
        &CommandSpec::new("cp").args([
            "-a",
            root.join("examples/proto/.").to_string_lossy().as_ref(),
            ctx.join("proto").to_string_lossy().as_ref(),
        ]),
    )?;

    let path_src = ctx.join("path-src");
    copy_file(root.join("Cargo.toml"), path_src.join("Cargo.toml"))?;
    copy_file(root.join("Cargo.lock"), path_src.join("Cargo.lock"))?;
    copy_file(
        root.join("rust-toolchain.toml"),
        path_src.join("rust-toolchain.toml"),
    )?;
    for member in ["buf-tools", "buf-toolchain", "examples", "xtask"] {
        run_command(
            root,
            &CommandSpec::new("cp").args(["-a", member, path_src.to_string_lossy().as_ref()]),
        )?;
    }
    if root.join(".cargo").is_dir() {
        run_command(
            root,
            &CommandSpec::new("cp").args(["-a", ".cargo", path_src.to_string_lossy().as_ref()]),
        )?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dest = ctx.join("entrypoint.sh");
        let mut perms = fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms)?;
    }

    Ok(ctx)
}

fn copy_file(from: PathBuf, to: PathBuf) -> Result<()> {
    fs::copy(&from, &to).with_context(|| format!("copy {} -> {}", from.display(), to.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AllImageArgs, AlpineImageArgs, DebianImageArgs};

    #[test]
    fn auto_order_has_both_concrete_engines_once() {
        assert_eq!(policy::IMAGE_AUTO_ORDER.len(), 2);
        assert!(policy::IMAGE_AUTO_ORDER.contains(&ImageEngine::Docker));
        assert!(policy::IMAGE_AUTO_ORDER.contains(&ImageEngine::Buildah));
        assert_ne!(policy::IMAGE_AUTO_ORDER[0], policy::IMAGE_AUTO_ORDER[1]);
    }

    #[test]
    fn debian_target_builds_only_debian() {
        let plans = plans(ImageTarget::Debian(DebianImageArgs {
            tag: "buf-rs-integration:debian-test".to_string(),
        }));
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].file, policy::IMAGE_DEBIAN_FILE);
        assert_eq!(plans[0].flavor, policy::IMAGE_DEBIAN_FLAVOR);
        assert_eq!(plans[0].tag, "buf-rs-integration:debian-test");
    }

    #[test]
    fn alpine_target_builds_only_alpine() {
        let plans = plans(ImageTarget::Alpine(AlpineImageArgs {
            tag: "buf-rs-integration:alpine-test".to_string(),
        }));
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].file, policy::IMAGE_ALPINE_FILE);
        assert_eq!(plans[0].flavor, policy::IMAGE_ALPINE_FLAVOR);
    }

    #[test]
    fn all_target_builds_each_owned_image_once_in_order() {
        let plans = plans(ImageTarget::All(AllImageArgs {
            debian_tag: "buf-rs-integration:debian".to_string(),
            alpine_tag: "buf-rs-integration:alpine".to_string(),
        }));
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].file, policy::IMAGE_DEBIAN_FILE);
        assert_eq!(plans[1].file, policy::IMAGE_ALPINE_FILE);
    }

    #[test]
    fn temporary_container_name_is_scoped() {
        assert!(temporary_name().starts_with("xtask-smoke-"));
    }
}
