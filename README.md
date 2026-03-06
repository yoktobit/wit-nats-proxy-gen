# rust-test workspace

This repository is now a workspace-first layout where reusable crates are the core of the project.

## Layout

- `crates/wit_nats_proxy` — runtime macros (`generate_wit_nats_consumer_proxy!`, `generate_wit_nats_provider_proxy!`)
- `crates/wit-wasmcloud-messaging-bindgen` — proc-macro crate (`generate_wit_nats_consumer_proxy_from_wit!`, `generate_wit_nats_provider_proxy_from_wit!`)
- `examples/acme-component` — example component crate using the macros

Examples live under `examples/` as separate crates, similar to other Rust workspaces.

## Build

```bash
cargo build --workspace
```
