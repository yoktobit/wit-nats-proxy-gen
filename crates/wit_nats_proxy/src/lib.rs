pub use serde;
pub use serde_json;
pub use wit_bindgen;

#[macro_export]
macro_rules! generate_wit_nats_proxy {
    (
        world: $world:literal,
        routes: [
            $(
                $proxy_fn:ident => {
                    wit_fn: $($wit_fn:ident)::+,
                    input: $($input_ty:ident)::+,
                    output: $output_ty:ty
                    $(, timeout_ms: $timeout_ms:expr)?
                    $(, subject: $subject:expr)?
                    $(,)?
                }
            ),+ $(,)?
        ]
        $(,)?
    ) => {
        generate_wit_nats_proxy!(
            world: $world,
            bindings_world: $world,
            global_prefix: "default",
            routes: [
                $(
                    $proxy_fn => {
                        wit_fn: $($wit_fn)::+,
                        input: $($input_ty)::+,
                        output: $output_ty
                        $(, timeout_ms: $timeout_ms)?
                        $(, subject: $subject)?
                    }
                ),+
            ],
        );
    };

    (
        world: $world:literal,
        bindings_world: $bindings_world:literal,
        routes: [
            $(
                $proxy_fn:ident => {
                    wit_fn: $($wit_fn:ident)::+,
                    input: $($input_ty:ident)::+,
                    output: $output_ty:ty
                    $(, timeout_ms: $timeout_ms:expr)?
                    $(, subject: $subject:expr)?
                    $(,)?
                }
            ),+ $(,)?
        ]
        $(,)?
    ) => {
        generate_wit_nats_proxy!(
            world: $world,
            bindings_world: $bindings_world,
            global_prefix: "default",
            routes: [
                $(
                    $proxy_fn => {
                        wit_fn: $($wit_fn)::+,
                        input: $($input_ty)::+,
                        output: $output_ty
                        $(, timeout_ms: $timeout_ms)?
                        $(, subject: $subject)?
                    }
                ),+
            ],
        );
    };

    (
        world: $world:literal,
        global_prefix: $global_prefix:literal,
        routes: [
            $(
                $proxy_fn:ident => {
                    wit_fn: $($wit_fn:ident)::+,
                    input: $($input_ty:ident)::+,
                    output: $output_ty:ty
                    $(, timeout_ms: $timeout_ms:expr)?
                    $(, subject: $subject:expr)?
                    $(,)?
                }
            ),+ $(,)?
        ]
        $(,)?
    ) => {
        generate_wit_nats_proxy!(
            world: $world,
            bindings_world: $world,
            global_prefix: $global_prefix,
            routes: [
                $(
                    $proxy_fn => {
                        wit_fn: $($wit_fn)::+,
                        input: $($input_ty)::+,
                        output: $output_ty
                        $(, timeout_ms: $timeout_ms)?
                        $(, subject: $subject)?
                    }
                ),+
            ],
        );
    };

    (
        world: $world:literal,
        bindings_world: $bindings_world:literal,
        global_prefix: $global_prefix:literal,
        routes: [
            $(
                $proxy_fn:ident => {
                    wit_fn: $($wit_fn:ident)::+,
                    input: $($input_ty:ident)::+,
                    output: $output_ty:ty
                    $(, timeout_ms: $timeout_ms:expr)?
                    $(, subject: $subject:expr)?
                    $(,)?
                }
            ),+ $(,)?
        ]
        $(,)?
    ) => {
        macro_rules! __route_timeout_ms {
            ($value:expr) => {
                $value
            };
            () => {
                10_000u32
            };
        }

        mod __wit_nats_proxy_bindings {
            use $crate::wit_bindgen as wit_bindgen;

            $crate::wit_bindgen::generate!({
                path: "wit",
                inline: r#"
                    package wit:nats-proxy@0.0.1;

                    world nats-request-world {
                        import wasmcloud:messaging/consumer@0.2.0;
                    }
                "#,
                generate_all,
            });
        }

        fn __nats_request<T, R>(
            subject: &str,
            timeout_ms: u32,
            input: &T,
        ) -> Result<R, String>
        where
            T: $crate::serde::Serialize,
            R: $crate::serde::de::DeserializeOwned,
        {
            let body = $crate::serde_json::to_vec(&input).map_err(|e| e.to_string())?;
            let res = __wit_nats_proxy_bindings::wasmcloud::messaging::consumer::request(
                subject,
                &body,
                timeout_ms,
            );

            match res {
                Ok(payload) => {
                    let response = $crate::serde_json::from_slice::<R>(&payload.body)
                        .map_err(|e| e.to_string())?;
                    Ok(response)
                }
                Err(e) => Err(e),
            }
        }

        $(
            fn $proxy_fn(
                input: $($input_ty)::+
            ) -> Result<$output_ty, String> {
                let subject = concat!("rpc.", $global_prefix, ".", stringify!($proxy_fn));
                $(let subject = $subject;)?

                __nats_request::<$($input_ty)::+, $output_ty>(
                    subject,
                    __route_timeout_ms!($($timeout_ms)?),
                    &input,
                )
            }
        )+
    };
}
