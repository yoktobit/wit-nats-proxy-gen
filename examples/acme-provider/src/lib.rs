use wit_wasmcloud_messaging_bindgen::generate_wit_nats_provider_proxy_from_wit;

generate_wit_nats_provider_proxy_from_wit!(
    world: "provider-world",
    generate_bindings: true,
    routes: [
        app_handle => {
            wit_fn: acme::app::external_function::handle,
            subject: "rpc.acme.handle",
        },
    ],
);

use crate::exports::acme::app::external_function::ExternalInput;

fn app_handle(input: ExternalInput) -> Result<String, String> {
    Ok(format!("provider received: {}", input.name))
}
