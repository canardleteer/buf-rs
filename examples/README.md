# buf-tools-examples

This package holds workspace-only examples for
[buf-tools](https://docs.rs/buf-tools). It is not published to crates.io.

Run examples from the repository root so paths such as `proto/` resolve.
The workspace README documents `buf_lint` and
`protoc_with_buf_plugins`. It also shows how to generate the
breaking-change baseline under `examples/proto/` before you run the
protoc example.

```bash
cargo run -p buf-tools-examples --example buf_lint
```
