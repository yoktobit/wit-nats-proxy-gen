wit_bindgen::generate!({
    path: "wit",
    world: "provider-world",
    additional_derives: [serde::Serialize, serde::Deserialize],
    generate_all,
});

wit_nats_proxy::generate_wit_nats_provider_proxy!(
    world: "provider-world",
    routes: [
        app_handle => {
            wit_fn: acme::app::acme_interface::handle,
            input: exports::acme::app::acme_interface::AcmeInput,
            output: String,
            subject: "rpc.acme.handle",
        },
    ],
);

use crate::exports::acme::app::acme_interface::AcmeInput;

fn app_handle(input: AcmeInput) -> Result<String, String> {
    Ok(format!("provider received: {}", input.name))
}
