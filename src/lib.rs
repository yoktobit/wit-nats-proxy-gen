mod wit_nats_proxy;

use wit_nats_proxy_macros::generate_wit_nats_proxy_from_wit;

generate_wit_nats_proxy_from_wit!(
    world: "acme-world-serde",
);

use crate::serde_world_bindings::acme::app::external_function::ExternalInput;
use crate::serde_world_bindings::exports::acme::app::acme_interface::{AcmeInput, Guest};

struct Component;

impl Guest for Component {
    fn handle(input: AcmeInput) -> Result<String, String> {
        let output = handle_nats(ExternalInput { name: input.name })?;

        Ok(output)
    }
}

serde_world_bindings::export!(Component with_types_in serde_world_bindings);