# buf-tools

Rust paths to the official **[Buf](https://github.com/bufbuild/buf)** CLI and bundled `protoc-gen-buf-*` plugins.

## What this crate does (read this first)

The **crates.io package does not contain** the executables. They are larger
than the registry upload limit, so on **first build** this crate’s **`build.rs`**
downloads them from **`bufbuild/buf` GitHub releases**, verifies
**[minisign](https://jedisct1.github.io/minisign/)** + **`sha256.txt`**, then
installs them under Cargo’s **`OUT_DIR`**. Same **upstream artifacts** you would
get from the official release — **pinned by this crate’s semver** — but
**fetched at compile time**, not shipped inside the `.crate` file.

## Network and authentication

- **HTTPS GET** to **`github.com`** only — **no GitHub token / PAT** required.
- Progress lines use **`cargo:warning=`** (often **~10% steps** per large file
  when `Content-Length` is present).

## Cache (default: no surprise re-downloads)

Downloaded blobs are stored under:

- **`$BUF_RS_CACHE_DIR/<semver-core>/<TARGET>/`** if **`BUF_RS_CACHE_DIR`**
  is set, else
- **`$XDG_CACHE_HOME/buf-tools/<semver-core>/<TARGET>/`** (with platform fallbacks
  via the **`dirs`** crate — see implementation).

After a successful download, **`cargo clean`** does **not** wipe this cache;
routine rebuilds reuse verified files and print a short **“using cached …”** message.

## Optional upstream source (`BUF_RS_INCLUDE_SOURCE`)

When **`BUF_RS_INCLUDE_SOURCE=1`** (or `true` / `yes`), **`build.rs`** also
downloads the **tagged source archive** from GitHub
(`archive/refs/tags/v{X.Y.Z}.tar.gz`), extracts it under the cache slot, and
sets **`BUF_RS_SOURCE_ROOT`** for this build.

**Integrity note:** Binaries are verified with Buf’s **`sha256.txt`** +
**minisign**. GitHub-generated **source tarballs are not covered by that
manifest** — treat them as an **audit / inspection convenience**, not the same
assurance level as the binary pipeline.
