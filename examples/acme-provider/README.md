# acme-provider-example

Minimal provider-side example for `wit_nats_proxy::generate_wit_nats_provider_proxy!`.

## What it demonstrates

- Generates local WIT bindings for the app world (`provider-world`)
- Generates wasmCloud messaging handler `Guest` wiring via provider macro
- Dispatches incoming broker messages by subject to your Rust function

## Generated routing

`src/lib.rs` maps subject `rpc.acme.handle` to `app_handle(input: AcmeInput)`.

## Build

```bash
cargo check -p acme-provider-example
```
