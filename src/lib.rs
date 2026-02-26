mod wit_nats_proxy;

use wit_nats_proxy_macros::generate_wit_nats_proxy_from_wit;

generate_wit_nats_proxy_from_wit!(
    serde_world: "acme-world-serde",
    no_serde_world: "acme-world-no-serde",
    routes: [
        handle_nats => {
            wit_fn: acme::app::external_function::handle,
        },
    ],
);

// TODO: Idee: Mit dem proevidence_-macro könnte er jetzt ads wit file auch lesen und für jede funktion einer world den proxy automatisch bauen
// TODO: Idee: Mit dem proevidence_-macro könnte er jetzt ads wit file auch lesen und für jede funktion einer world den proxy automatisch bauen
// TODO: Idee: Mit dem proevidence_-macro könnte er jetzt ads wit file auch lesen und für jede funktion einer world den proxy automatisch bauen
// dann brauchen wir nur eine world dafür, nicht mehr unbedingt als serde_world, sondern als nats_world

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