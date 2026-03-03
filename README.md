# rust-test workspace

This repository is now a workspace-first layout where reusable crates are the core of the project.

## Layout

- `crates/wit_nats_proxy` — runtime `generate_wit_nats_proxy!` macro crate
- `crates/wit_nats_proxy_macros` — proc-macro crate (`generate_wit_nats_proxy_from_wit!`)
- `examples/acme-component` — example component crate using the macros

Examples live under `examples/` as separate crates, similar to other Rust workspaces.

## Build

```bash
cargo build --workspace
```
