use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::path::PathBuf;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{braced, bracketed, parse_macro_input, Expr, Ident, LitStr, Path, Result, Token};
use wit_parser::{Resolve, Type, TypeDefKind, WorldItem, WorldKey};

struct RouteSpec {
    proxy_fn: Ident,
    wit_fn: Path,
    timeout_ms: Option<Expr>,
    subject: Option<LitStr>,
}

struct RouteOverrideSpec {
    proxy_fn: Ident,
    timeout_ms: Option<Expr>,
    subject: Option<LitStr>,
}

impl Parse for RouteSpec {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let proxy_fn: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;

        let content;
        braced!(content in input);

        let mut wit_fn: Option<Path> = None;
        let mut timeout_ms: Option<Expr> = None;
        let mut subject: Option<LitStr> = None;

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "wit_fn" => wit_fn = Some(content.parse()?),
                "timeout_ms" => timeout_ms = Some(content.parse()?),
                "subject" => subject = Some(content.parse()?),
                _ => return Err(syn::Error::new(key.span(), "unknown route field")),
            }

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        let proxy_span = proxy_fn.span();

        Ok(Self {
            proxy_fn,
            wit_fn: wit_fn.ok_or_else(|| syn::Error::new(proxy_span, "missing wit_fn"))?,
            timeout_ms,
            subject,
        })
    }
}

impl Parse for RouteOverrideSpec {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let proxy_fn: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;

        let content;
        braced!(content in input);

        let mut timeout_ms: Option<Expr> = None;
        let mut subject: Option<LitStr> = None;

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "timeout_ms" => timeout_ms = Some(content.parse()?),
                "subject" => subject = Some(content.parse()?),
                _ => return Err(syn::Error::new(key.span(), "unknown route override field")),
            }

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            proxy_fn,
            timeout_ms,
            subject,
        })
    }
}

struct ProxyConfig {
    serde_mod: Option<Ident>,
    world: LitStr,
    global_prefix: Option<LitStr>,
    wit_path: Option<LitStr>,
    routes: Option<Vec<RouteSpec>>,
    route_overrides: Option<Vec<RouteOverrideSpec>>,
}

impl Parse for ProxyConfig {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut serde_mod: Option<Ident> = None;
        let mut world: Option<LitStr> = None;
        let mut global_prefix: Option<LitStr> = None;
        let mut wit_path: Option<LitStr> = None;
        let mut routes: Option<Vec<RouteSpec>> = None;
        let mut route_overrides: Option<Vec<RouteOverrideSpec>> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "serde_mod" => serde_mod = Some(input.parse()?),
                "world" => world = Some(input.parse()?),
                "global_prefix" => global_prefix = Some(input.parse()?),
                "wit_path" => wit_path = Some(input.parse()?),
                "routes" => {
                    let content;
                    bracketed!(content in input);
                    let mut parsed = Vec::new();
                    while !content.is_empty() {
                        parsed.push(content.parse()?);
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    routes = Some(parsed);
                }
                "route_overrides" => {
                    let content;
                    bracketed!(content in input);
                    let mut parsed = Vec::new();
                    while !content.is_empty() {
                        parsed.push(content.parse()?);
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    route_overrides = Some(parsed);
                }
                _ => return Err(syn::Error::new(key.span(), "unknown config field")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            serde_mod,
            world: world.ok_or_else(|| syn::Error::new(Span::call_site(), "missing world"))?,
            global_prefix,
            wit_path,
            routes,
            route_overrides,
        })
    }
}

#[proc_macro]
pub fn generate_wit_nats_proxy_from_wit(input: TokenStream) -> TokenStream {
    let cfg = parse_macro_input!(input as ProxyConfig);

    match expand(cfg) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand(cfg: ProxyConfig) -> Result<TokenStream2> {
    let world = cfg.world;

    let serde_mod = cfg
        .serde_mod
        .unwrap_or_else(|| Ident::new("serde_world_bindings", Span::call_site()));
    let global_prefix = cfg
        .global_prefix
        .unwrap_or_else(|| LitStr::new("default", Span::call_site()));

    let wit_rel = cfg
        .wit_path
        .unwrap_or_else(|| LitStr::new("wit/world.wit", Span::call_site()));

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| syn::Error::new(Span::call_site(), "CARGO_MANIFEST_DIR is not set"))?;
    let wit_file = PathBuf::from(manifest_dir).join(wit_rel.value());

    let wit_input = if wit_file.is_dir() {
        wit_file.clone()
    } else {
        wit_file
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| wit_file.clone())
    };

    let mut resolve = Resolve::default();
    resolve.push_path(&wit_input).map_err(|e| {
        syn::Error::new(
            Span::call_site(),
            format!("failed to parse WIT at {}: {e}", wit_input.display()),
        )
    })?;

    let world_name = world.value();
    let world_id = find_world_id(&resolve, &world_name).ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            format!("world '{world_name}' not found in {}", wit_input.display()),
        )
    })?;

    let route_specs = if let Some(routes) = cfg.routes {
        if routes.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "routes must not be empty when provided",
            ));
        }
        routes
    } else {
        infer_routes_from_world(&resolve, world_id)?
    };

    let route_specs = apply_route_overrides(route_specs, cfg.route_overrides)?;

    let mut route_tokens = Vec::new();

    for route in route_specs {
        let parts: Vec<String> = route
            .wit_fn
            .segments
            .iter()
            .map(|seg| seg.ident.to_string())
            .collect();

        if parts.len() < 4 {
            return Err(syn::Error::new(
                route.wit_fn.span(),
                "wit_fn must look like namespace::package::interface::function",
            ));
        }

        let interface_mod = parts[2].clone();
        let function_name = parts[3].clone();
        let interface_wit = interface_mod.replace('_', "-");

        let function = resolve_world_function(&resolve, world_id, &interface_wit, &function_name)
            .map_err(|msg| syn::Error::new(route.wit_fn.span(), msg))?;

        if function.params.len() != 1 {
            return Err(syn::Error::new(
                route.wit_fn.span(),
                format!(
                    "wit_fn '{}' must have exactly one parameter, found {}",
                    function_name,
                    function.params.len()
                ),
            ));
        }

        let input_ty = wit_type_to_tokens(
            &resolve,
            &function.params[0].ty,
            &parts[0],
            &parts[1],
            &interface_mod,
        )?;

        let output_ty = results_ok_type_to_tokens(
            &resolve,
            &function.result,
            &parts[0],
            &parts[1],
            &interface_mod,
        )?;

        let proxy_fn = route.proxy_fn;
        let wit_fn = route.wit_fn;

        let timeout_tokens = if let Some(timeout_ms) = route.timeout_ms {
            quote! { , timeout_ms: #timeout_ms }
        } else {
            quote! {}
        };

        let subject_tokens = if let Some(subject) = route.subject {
            quote! { , subject: #subject }
        } else {
            quote! {}
        };

        route_tokens.push(quote! {
            #proxy_fn => {
                wit_fn: #wit_fn,
                input: #input_ty,
                output: #output_ty
                #timeout_tokens
                #subject_tokens
            }
        });
    }

    Ok(quote! {
        generate_wit_nats_proxy!(
            serde_mod: #serde_mod,
            world: #world,
            global_prefix: #global_prefix,
            routes: [
                #(#route_tokens),*
            ],
        );
    })
}

fn find_world_id(resolve: &Resolve, world_name: &str) -> Option<wit_parser::WorldId> {
    for (_, pkg) in resolve.packages.iter() {
        if let Some(world_id) = pkg.worlds.get(world_name) {
            return Some(*world_id);
        }
    }

    None
}

fn resolve_world_function<'a>(
    resolve: &'a Resolve,
    world_id: wit_parser::WorldId,
    interface_name: &str,
    function_name: &str,
) -> std::result::Result<&'a wit_parser::Function, String> {
    let world = &resolve.worlds[world_id];

    let available_world_interfaces: Vec<String> = world
        .imports
        .iter()
        .chain(world.exports.iter())
        .filter_map(|(key, item)| match (key, item) {
            (WorldKey::Name(name), WorldItem::Interface { .. }) => Some(name.to_string()),
            _ => None,
        })
        .collect();

    let interface_id_from_world = world
        .imports
        .iter()
        .chain(world.exports.iter())
        .find_map(|(key, item)| match (key, item) {
            (WorldKey::Name(name), WorldItem::Interface { id, .. }) if name == interface_name => {
                Some(*id)
            }
            _ => None,
        })
        ;

    let interface_id = if let Some(id) = interface_id_from_world {
        id
    } else if let Some(pkg_id) = world.package {
        let pkg = &resolve.packages[pkg_id];
        if let Some(id) = pkg.interfaces.get(interface_name) {
            *id
        } else if let Some((_, id)) = pkg
            .interfaces
            .iter()
            .find(|(name, _)| name.ends_with(&format!("/{interface_name}")))
        {
            *id
        } else {
            let known: Vec<String> = pkg.interfaces.keys().cloned().collect();
            return Err(format!(
                "interface '{}' not found in world '{}' or package interfaces; world interfaces: [{}]; package interfaces: [{}]",
                interface_name,
                world.name,
                available_world_interfaces.join(", "),
                known.join(", "),
            ));
        }
    } else {
        return Err(format!(
            "interface '{}' not found in world '{}'; world interfaces: [{}]",
            interface_name,
            world.name,
            available_world_interfaces.join(", "),
        ));
    };

    let interface = &resolve.interfaces[interface_id];
    interface
        .functions
        .get(function_name)
        .ok_or_else(|| {
            format!(
                "function '{}' not found in interface '{}'",
                function_name, interface_name
            )
        })
}

fn results_ok_type_to_tokens(
    resolve: &Resolve,
    result: &Option<Type>,
    ns: &str,
    pkg: &str,
    interface_mod: &str,
) -> Result<TokenStream2> {
    match result {
        Some(ty) => extract_ok_from_type(resolve, ty, ns, pkg, interface_mod),
        None => Ok(quote! { () }),
    }
}

fn extract_ok_from_type(
    resolve: &Resolve,
    ty: &Type,
    ns: &str,
    pkg: &str,
    interface_mod: &str,
) -> Result<TokenStream2> {
    if let Type::Id(type_id) = ty {
        let type_def = &resolve.types[*type_id];
        if let TypeDefKind::Result(result_ty) = &type_def.kind {
            if let Some(ok_ty) = &result_ty.ok {
                return wit_type_to_tokens(resolve, ok_ty, ns, pkg, interface_mod);
            }

            return Ok(quote! { () });
        }
    }

    wit_type_to_tokens(resolve, ty, ns, pkg, interface_mod)
}

fn wit_type_to_tokens(
    resolve: &Resolve,
    ty: &Type,
    ns: &str,
    pkg: &str,
    interface_mod: &str,
) -> Result<TokenStream2> {
    match ty {
        Type::Bool => Ok(quote! { bool }),
        Type::U8 => Ok(quote! { u8 }),
        Type::U16 => Ok(quote! { u16 }),
        Type::U32 => Ok(quote! { u32 }),
        Type::U64 => Ok(quote! { u64 }),
        Type::S8 => Ok(quote! { i8 }),
        Type::S16 => Ok(quote! { i16 }),
        Type::S32 => Ok(quote! { i32 }),
        Type::S64 => Ok(quote! { i64 }),
        Type::F32 => Ok(quote! { f32 }),
        Type::F64 => Ok(quote! { f64 }),
        Type::Char => Ok(quote! { char }),
        Type::String => Ok(quote! { String }),
        Type::ErrorContext => Ok(quote! { String }),
        Type::Id(type_id) => wit_type_def_to_tokens(resolve, *type_id, ns, pkg, interface_mod),
    }
}

fn wit_type_def_to_tokens(
    resolve: &Resolve,
    type_id: wit_parser::TypeId,
    ns: &str,
    pkg: &str,
    interface_mod: &str,
) -> Result<TokenStream2> {
    let type_def = &resolve.types[type_id];

    match &type_def.kind {
        TypeDefKind::Type(inner) => wit_type_to_tokens(resolve, inner, ns, pkg, interface_mod),
        TypeDefKind::Option(inner) => {
            let inner_ty = wit_type_to_tokens(resolve, inner, ns, pkg, interface_mod)?;
            Ok(quote! { Option<#inner_ty> })
        }
        TypeDefKind::List(inner) => {
            let inner_ty = wit_type_to_tokens(resolve, inner, ns, pkg, interface_mod)?;
            Ok(quote! { Vec<#inner_ty> })
        }
        TypeDefKind::Tuple(tuple) => {
            let mut tuple_items = Vec::with_capacity(tuple.types.len());
            for item in &tuple.types {
                tuple_items.push(wit_type_to_tokens(resolve, item, ns, pkg, interface_mod)?);
            }
            Ok(quote! { ( #(#tuple_items),* ) })
        }
        TypeDefKind::Result(result_ty) => {
            let ok_ty = if let Some(ok) = &result_ty.ok {
                wit_type_to_tokens(resolve, ok, ns, pkg, interface_mod)?
            } else {
                quote! { () }
            };
            let err_ty = if let Some(err) = &result_ty.err {
                wit_type_to_tokens(resolve, err, ns, pkg, interface_mod)?
            } else {
                quote! { () }
            };
            Ok(quote! { Result<#ok_ty, #err_ty> })
        }
        TypeDefKind::Future(inner) => {
            let value_ty = if let Some(value) = inner {
                wit_type_to_tokens(resolve, value, ns, pkg, interface_mod)?
            } else {
                quote! { () }
            };
            Ok(quote! { #value_ty })
        }
        TypeDefKind::Stream(stream) => {
            let element_ty = if let Some(value) = stream {
                wit_type_to_tokens(resolve, value, ns, pkg, interface_mod)?
            } else {
                quote! { () }
            };
            Ok(quote! { Vec<#element_ty> })
        }
        _ => named_type_path(type_def.name.as_deref(), ns, pkg, interface_mod),
    }
}

fn named_type_path(name: Option<&str>, ns: &str, pkg: &str, interface_mod: &str) -> Result<TokenStream2> {
    let Some(type_name) = name else {
        return Err(syn::Error::new(
            Span::call_site(),
            "encountered unnamed WIT type that cannot be mapped to a Rust path",
        ));
    };

    let ns_ident = format_ident!("{}", ns);
    let pkg_ident = format_ident!("{}", pkg);
    let interface_ident = format_ident!("{}", interface_mod);
    let type_ident = format_ident!("{}", to_upper_camel(type_name));

    Ok(quote! { #ns_ident::#pkg_ident::#interface_ident::#type_ident })
}

fn to_upper_camel(name: &str) -> String {
    name
        .split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn infer_routes_from_world(resolve: &Resolve, world_id: wit_parser::WorldId) -> Result<Vec<RouteSpec>> {
    let world = &resolve.worlds[world_id];
    let world_pkg = world.package;

    let mut routes = Vec::new();

    for item in world.imports.values() {
        let interface_id = match item {
            WorldItem::Interface { id, .. } => *id,
            _ => continue,
        };

        let interface = &resolve.interfaces[interface_id];
        let interface_name = interface
            .name
            .as_deref()
            .unwrap_or("interface")
            .to_string();

        if interface.package.is_some() && interface.package != world_pkg {
            continue;
        }

        let iface_ident = format_ident!("{}", sanitize_ident_segment(&interface_name));

        let (ns_ident, pkg_ident) = if let Some(pkg_id) = interface.package {
            let pkg = &resolve.packages[pkg_id];
            (
                format_ident!("{}", sanitize_ident_segment(&pkg.name.namespace.to_string())),
                format_ident!("{}", sanitize_ident_segment(&pkg.name.name.to_string())),
            )
        } else if let Some(pkg_id) = world_pkg {
            let pkg = &resolve.packages[pkg_id];
            (
                format_ident!("{}", sanitize_ident_segment(&pkg.name.namespace.to_string())),
                format_ident!("{}", sanitize_ident_segment(&pkg.name.name.to_string())),
            )
        } else {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("interface '{interface_name}' has no package information"),
            ));
        };

        for fn_name in interface.functions.keys() {
            let rust_fn_name = sanitize_ident_segment(fn_name);
            let fn_ident = format_ident!("{}", rust_fn_name);
            let wit_fn = syn::parse2::<Path>(quote! { #ns_ident::#pkg_ident::#iface_ident::#fn_ident })
                .map_err(|e| syn::Error::new(Span::call_site(), format!("failed to build inferred wit_fn path: {e}")))?;

            let proxy_fn = Ident::new(&(rust_fn_name.clone() + "_nats"), Span::call_site());

            routes.push(RouteSpec {
                proxy_fn,
                wit_fn,
                timeout_ms: None,
                subject: None,
            });
        }
    }

    if routes.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            format!(
                "no inferable routes found in world '{}' (expected imported interfaces from the same package)",
                world.name
            ),
        ));
    }

    Ok(routes)
}

fn sanitize_ident_segment(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            output.push(c);
        } else {
            output.push('_');
        }
    }
    output
}

fn apply_route_overrides(
    mut routes: Vec<RouteSpec>,
    overrides: Option<Vec<RouteOverrideSpec>>,
) -> Result<Vec<RouteSpec>> {
    let Some(overrides) = overrides else {
        return Ok(routes);
    };

    for override_spec in overrides {
        let override_name = override_spec.proxy_fn.to_string();
        let mut matched = false;

        for route in &mut routes {
            if route.proxy_fn == override_spec.proxy_fn {
                if override_spec.timeout_ms.is_some() {
                    route.timeout_ms = override_spec.timeout_ms.clone();
                }
                if override_spec.subject.is_some() {
                    route.subject = override_spec.subject.clone();
                }
                matched = true;
                break;
            }
        }

        if !matched {
            return Err(syn::Error::new(
                override_spec.proxy_fn.span(),
                format!("route_overrides entry '{override_name}' did not match any route"),
            ));
        }
    }

    Ok(routes)
}
