# wit-wasmcloud-messaging-bindgen

Procedural macro helpers to generate NATS proxy functions from WIT worlds.

## Macros

```rust
generate_wit_nats_consumer_proxy_from_wit!(...)
generate_wit_nats_provider_proxy_from_wit!(...)
```

These macros read your WIT definitions, resolve function signatures, and expand to runtime macros in `wit_nats_proxy`.

- `generate_wit_nats_consumer_proxy_from_wit!` generates NATS request/response consumer helpers.
- `generate_wit_nats_provider_proxy_from_wit!` generates a provider `Guest` implementation plus a public `handle` dispatcher.
- `generate_wit_nats_proxy_from_wit!` remains available as a compatibility alias to the consumer macro.

## Required dependencies

Your crate should depend on both crates:

```toml
[dependencies]
wit_nats_proxy = { path = "../../crates/wit_nats_proxy" }
wit-wasmcloud-messaging-bindgen = { path = "../../crates/wit-wasmcloud-messaging-bindgen" }
```

## What it generates

- `serde`-enabled bindings generated in place at the macro invocation site
- One proxy function per route (explicit or inferred), each returning `Result<Output, String>`
- NATS request/response wiring via `wasmcloud:messaging/consumer@0.2.0`

## Required input

```rust
generate_wit_nats_consumer_proxy_from_wit!(
    world: "proxy-schema",
    bindings_world: "hello",
    wit_path: "../../wit/world.wit",
);
```

- `world`: WIT world name used for route inference and signature resolution.
- `bindings_world` (optional): WIT world used by runtime `wit_bindgen` generation (defaults to `world`).

## Optional input

- `global_prefix`: Subject prefix for default subjects (default: `"default"`)
- `wit_path`: Path to your WIT entry (default: `"wit/world.wit"`)
- `generate_bindings`: Auto-emit `wit_bindgen::generate!` for `bindings_world` (default: `true`)
- `routes`: Explicit route list
- `route_overrides`: Override timeout/subject for existing routes (explicit or inferred)
- `bindings_world`: Runtime bindings world (when omitted, `world` is used)

## Route behavior

### 1) Explicit routes

```rust
generate_wit_nats_consumer_proxy_from_wit!(
    world: "acme-world-serde",
    routes: [
        handle_nats => {
            wit_fn: acme::app::external_function::handle,
            timeout_ms: 5_000,
            subject: "rpc.custom.handle",
        },
    ],
);
```

Route fields:

- `wit_fn` (required): path like `namespace::package::interface::function`
- `timeout_ms` (optional): request timeout in ms (default: `10_000`)
- `subject` (optional): full NATS subject (default: `rpc.<global_prefix>.<proxy_fn>`)

### 2) Inferred routes (when `routes` is omitted)

If `routes` is not provided, routes are inferred from imported or exported interfaces in the selected `world`.

Generated proxy naming rule:

- `<wit_function_name>_nats`

Example: WIT function `handle` becomes proxy `handle_nats`.

## Route overrides

Use `route_overrides` to customize only selected inferred (or explicit) routes:

```rust
generate_wit_nats_consumer_proxy_from_wit!(
    world: "acme-world-serde",
    route_overrides: [
        handle_nats => {
            timeout_ms: 15_000,
            subject: "rpc.acme.external.handle",
        },
    ],
);
```

Override fields:

- `timeout_ms` (optional)
- `subject` (optional)

If an override name does not match any route, macro expansion fails with a compile-time error.

## Expected function signature

Each routed WIT function must have exactly one parameter.

- Input type is inferred from that parameter
- Output type is inferred from the WIT result (uses `Ok` type for `result<T, E>`)

## Minimal end-to-end example

```rust
use wit_wasmcloud_messaging_bindgen::generate_wit_nats_consumer_proxy_from_wit;

generate_wit_nats_consumer_proxy_from_wit!(
    world: "acme-world-serde",
);

use crate::acme::app::external_function::ExternalInput;
use crate::exports::acme::app::acme_interface::{AcmeInput, Guest};

struct Component;

impl Guest for Component {
    fn handle(input: AcmeInput) -> Result<String, String> {
        handle_nats(ExternalInput { name: input.name })
    }
}

export!(Component with_types_in self);
```

## Notes

- `wit_path` is resolved relative to `CARGO_MANIFEST_DIR`.
- By default, no separate `wit_bindgen::generate!` call is required.
- Set `generate_bindings: false` only if you already generate WIT bindings manually.
- The macro uses `wit_parser` at compile time; errors are surfaced as Rust compile errors.
- For defaults, subjects are built as: `rpc.<global_prefix>.<proxy_fn>`.

## WIT requirements

To use generated NATS proxy functions, your component runtime world must include:

```wit
import wasmcloud:messaging/consumer@0.2.0;
```

For provider generation, your world must include the wasmCloud messaging handler export:

```wit
export wasmcloud:messaging/handler@0.2.0;
```

Example:

```wit
world hello {
    export wasi:http/incoming-handler@0.2.2;
    import wasmcloud:messaging/consumer@0.2.0;
}

For route inference/signature extraction, prefer a separate schema world that exports your app interface:

```wit
world proxy-schema {
    export outer-space-handler;
}
```
```

Also ensure your component has the dependency package in `wit/deps`:

- `wit/deps/wasmcloud-messaging-0.2.0/package.wit`

Without this import + dep package, `cargo check` / `wash build` can fail with errors like `package 'wasmcloud:messaging@0.2.0' not found`.
