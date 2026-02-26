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
}

impl Parse for RouteSpec {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let proxy_fn: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;

        let content;
        braced!(content in input);

        let mut wit_fn: Option<Path> = None;
        let mut timeout_ms: Option<Expr> = None;

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "wit_fn" => wit_fn = Some(content.parse()?),
                "timeout_ms" => timeout_ms = Some(content.parse()?),
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
        })
    }
}

struct ProxyConfig {
    serde_mod: Option<Ident>,
    serde_world: LitStr,
    no_serde_mod: Option<Ident>,
    no_serde_world: LitStr,
    global_prefix: Option<LitStr>,
    wit_path: Option<LitStr>,
    routes: Vec<RouteSpec>,
}

impl Parse for ProxyConfig {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut serde_mod: Option<Ident> = None;
        let mut serde_world: Option<LitStr> = None;
        let mut no_serde_mod: Option<Ident> = None;
        let mut no_serde_world: Option<LitStr> = None;
        let mut global_prefix: Option<LitStr> = None;
        let mut wit_path: Option<LitStr> = None;
        let mut routes: Option<Vec<RouteSpec>> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "serde_mod" => serde_mod = Some(input.parse()?),
                "serde_world" => serde_world = Some(input.parse()?),
                "no_serde_mod" => no_serde_mod = Some(input.parse()?),
                "no_serde_world" => no_serde_world = Some(input.parse()?),
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
                _ => return Err(syn::Error::new(key.span(), "unknown config field")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            serde_mod,
            serde_world: serde_world.ok_or_else(|| syn::Error::new(Span::call_site(), "missing serde_world"))?,
            no_serde_mod,
            no_serde_world: no_serde_world
                .ok_or_else(|| syn::Error::new(Span::call_site(), "missing no_serde_world"))?,
            global_prefix,
            wit_path,
            routes: routes.ok_or_else(|| syn::Error::new(Span::call_site(), "missing routes"))?,
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
    let serde_world = cfg.serde_world;
    let no_serde_world = cfg.no_serde_world;

    let serde_mod = cfg
        .serde_mod
        .unwrap_or_else(|| Ident::new("serde_world_bindings", Span::call_site()));
    let no_serde_mod = cfg
        .no_serde_mod
        .unwrap_or_else(|| Ident::new("no_serde_world_bindings", Span::call_site()));
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

    let serde_world_name = serde_world.value();
    let serde_world_id = find_world_id(&resolve, &serde_world_name).ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            format!("world '{serde_world_name}' not found in {}", wit_input.display()),
        )
    })?;

    let mut route_tokens = Vec::new();

    for route in cfg.routes {
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

        let function = resolve_world_function(&resolve, serde_world_id, &interface_wit, &function_name)
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

        route_tokens.push(quote! {
            #proxy_fn => {
                wit_fn: #wit_fn,
                input: #input_ty,
                output: #output_ty
                #timeout_tokens
            }
        });
    }

    Ok(quote! {
        generate_wit_nats_proxy!(
            serde_mod: #serde_mod,
            serde_world: #serde_world,
            no_serde_mod: #no_serde_mod,
            no_serde_world: #no_serde_world,
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
