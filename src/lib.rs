mod wit_nats_proxy;

generate_wit_nats_proxy!(
    serde_world: "acme-world-serde",
    no_serde_world: "acme-world-no-serde",
    routes: [
        handle_nats => {
            wit_fn: acme::app::external_function::handle,
            input: acme::app::external_function::ExternalInput,
            output: String,
        },
    ],
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