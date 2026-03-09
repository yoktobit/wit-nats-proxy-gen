# acme-provider-example

Minimal provider-side example for `generate_wit_nats_provider_proxy_from_wit!`.

## What it demonstrates

- Auto-generates local WIT bindings for the app world (`provider-world`)
- Generates wasmCloud messaging handler `Guest` wiring via provider macro
- Dispatches incoming broker messages by subject to your Rust function

## Generated routing

`src/lib.rs` maps subject `rpc.acme.handle` to `app_handle(input: AcmeInput)`.

No separate `wit_bindgen::generate!` call is needed.

## Build

```bash
cargo check -p acme-provider-example
```
