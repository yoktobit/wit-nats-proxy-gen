use wit_wasmcloud_messaging_bindgen::generate_wit_nats_consumer_proxy_from_wit;

wit_bindgen::generate!({
    path: "wit",
    world: "acme-world-serde",
    additional_derives: [serde::Serialize, serde::Deserialize],
    generate_all,
});

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
