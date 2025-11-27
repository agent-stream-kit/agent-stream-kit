//! Procedural macros for agent-stream-kit.
//!
//! Provides an attribute to declare agent metadata alongside the agent type and
//! generate the registration boilerplate.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Expr, ItemStruct, Lit, LitStr, Meta, MetaList, MetaNameValue, parse_macro_input,
    punctuated::Punctuated, token::Comma,
};

#[proc_macro_attribute]
pub fn askit(attr: TokenStream, item: TokenStream) -> TokenStream {
    askit_agent(attr, item)
}

/// Declare agent metadata and generate `agent_definition` / `register` helpers.
///
/// Example:
/// ```rust,ignore
/// #[askit_agent(
///     kind = "Board",
///     title = "Board In",
///     category = "Core",
///     inputs = ["*"],
///     string_config(
///         name = CONFIG_BOARD_NAME,
///         default = "",
///         title = "Board Name",
///         description = "* = source kind"
///     )
/// )]
/// struct BoardInAgent { /* ... */ }
/// ```
#[proc_macro_attribute]
pub fn askit_agent(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated<Meta, Comma>::parse_terminated);
    let item_struct = parse_macro_input!(item as ItemStruct);

    match expand_askit_agent(args, item_struct) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

struct AgentArgs {
    kind: Option<LitStr>,
    name: Option<LitStr>,
    title: Option<LitStr>,
    description: Option<LitStr>,
    category: Option<LitStr>,
    inputs: Vec<LitStr>,
    outputs: Vec<LitStr>,
    string_config: Option<StringConfig>,
}

#[derive(Default)]
struct StringConfig {
    name: Option<Expr>,
    default: Option<Expr>,
    title: Option<LitStr>,
    description: Option<LitStr>,
}

fn expand_askit_agent(
    args: Punctuated<Meta, Comma>,
    item: ItemStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut parsed = AgentArgs {
        kind: None,
        name: None,
        title: None,
        description: None,
        category: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        string_config: None,
    };

    for meta in args {
        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("kind") => {
                parsed.kind = Some(expect_lit_str(nv)?);
            }
            Meta::NameValue(nv) if nv.path.is_ident("name") => {
                parsed.name = Some(expect_lit_str(nv)?);
            }
            Meta::NameValue(nv) if nv.path.is_ident("title") => {
                parsed.title = Some(expect_lit_str(nv)?);
            }
            Meta::NameValue(nv) if nv.path.is_ident("description") => {
                parsed.description = Some(expect_lit_str(nv)?);
            }
            Meta::NameValue(nv) if nv.path.is_ident("category") => {
                parsed.category = Some(expect_lit_str(nv)?);
            }
            Meta::NameValue(nv) if nv.path.is_ident("inputs") => {
                parsed.inputs = parse_lit_str_array(nv.value)?;
            }
            Meta::NameValue(nv) if nv.path.is_ident("outputs") => {
                parsed.outputs = parse_lit_str_array(nv.value)?;
            }
            Meta::List(ml) if ml.path.is_ident("inputs") => {
                parsed.inputs = collect_lit_strs(ml)?;
            }
            Meta::List(ml) if ml.path.is_ident("outputs") => {
                parsed.outputs = collect_lit_strs(ml)?;
            }
            Meta::List(ml) if ml.path.is_ident("string_config") => {
                parsed.string_config = Some(parse_string_config(ml)?);
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "unsupported askit_agent argument",
                ));
            }
        }
    }

    let ident = &item.ident;
    let generics = item.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let kind = parsed
        .kind
        .ok_or_else(|| syn::Error::new(Span::call_site(), "askit_agent: missing `kind`"))?;
    let name_tokens = parsed.name.map(|n| quote! { #n }).unwrap_or_else(|| {
        quote! { concat!(module_path!(), "::", stringify!(#ident)) }
    });

    let title = parsed.title.map(|t| quote! { .title(#t) });
    let description = parsed.description.map(|d| quote! { .description(#d) });
    let category = parsed.category.map(|c| quote! { .category(#c) });

    let inputs = if parsed.inputs.is_empty() {
        quote! {}
    } else {
        let values = parsed.inputs;
        quote! { .inputs(vec![#(#values),*]) }
    };

    let outputs = if parsed.outputs.is_empty() {
        quote! {}
    } else {
        let values = parsed.outputs;
        quote! { .outputs(vec![#(#values),*]) }
    };

    let string_config = parsed
        .string_config
        .map(|cfg| {
            let name = cfg.name.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "string_config missing `name`")
            })?;
            let default = cfg.default.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "string_config missing `default`")
            })?;
            let title = cfg.title.map(|t| quote! { let entry = entry.title(#t); });
            let description = cfg
                .description
                .map(|d| quote! { let entry = entry.description(#d); });

            Ok::<_, syn::Error>(quote! {
                .string_config_with(#name, #default, |entry| {
                    let entry = entry;
                    #title
                    #description
                    entry
                })
            })
        })
        .transpose()?;

    let definition_builder = quote! {
        ::agent_stream_kit::AgentDefinition::new(
            #kind,
            #name_tokens,
            Some(::agent_stream_kit::new_agent_boxed::<#ident>),
        )
        #title
        #description
        #category
        #inputs
        #outputs
        #string_config
    };

    let expanded = quote! {
        #item

        impl #impl_generics #ident #ty_generics #where_clause {
            pub fn agent_definition() -> ::agent_stream_kit::AgentDefinition {
                #definition_builder
            }

            pub fn register(askit: &::agent_stream_kit::ASKit) {
                askit.register_agent(Self::agent_definition());
            }
        }

        ::agent_stream_kit::inventory::submit! {
            ::agent_stream_kit::AgentRegistration {
                build: || #definition_builder,
            }
        }
    };

    Ok(expanded)
}

fn expect_lit_str(nv: MetaNameValue) -> syn::Result<LitStr> {
    match nv.value {
        Expr::Lit(expr_lit) => match expr_lit.lit {
            Lit::Str(s) => Ok(s),
            other => Err(syn::Error::new_spanned(other, "expected string literal")),
        },
        other => Err(syn::Error::new_spanned(other, "expected string literal")),
    }
}

fn collect_lit_strs(list: MetaList) -> syn::Result<Vec<LitStr>> {
    let values = list.parse_args_with(Punctuated::<LitStr, Comma>::parse_terminated)?;
    Ok(values.into_iter().collect())
}

fn parse_lit_str_array(expr: Expr) -> syn::Result<Vec<LitStr>> {
    if let Expr::Array(arr) = expr {
        let mut out = Vec::new();
        for elem in arr.elems {
            match elem {
                Expr::Lit(expr_lit) => match expr_lit.lit {
                    Lit::Str(s) => out.push(s),
                    other => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "inputs/outputs expect string literals",
                        ));
                    }
                },
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "inputs/outputs expect string literals",
                    ));
                }
            }
        }
        Ok(out)
    } else {
        Err(syn::Error::new_spanned(
            expr,
            "inputs/outputs expect array of string literals",
        ))
    }
}

fn parse_string_config(list: MetaList) -> syn::Result<StringConfig> {
    let mut cfg = StringConfig::default();
    let nested = list.parse_args_with(Punctuated::<Meta, Comma>::parse_terminated)?;

    for meta in nested {
        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("name") => {
                cfg.name = Some(match &nv.value {
                    Expr::Lit(expr_lit) => match &expr_lit.lit {
                        Lit::Str(s) => syn::parse_str::<Expr>(&s.value())?,
                        _ => nv.value.clone(),
                    },
                    _ => nv.value.clone(),
                });
            }
            Meta::NameValue(nv) if nv.path.is_ident("default") => {
                cfg.default = Some(nv.value.clone());
            }
            Meta::NameValue(nv) if nv.path.is_ident("title") => {
                cfg.title = Some(expect_lit_str(nv)?);
            }
            Meta::NameValue(nv) if nv.path.is_ident("description") => {
                cfg.description = Some(expect_lit_str(nv)?);
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "string_config supports name, default, title, description",
                ));
            }
        }
    }
    Ok(cfg)
}
