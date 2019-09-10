//! `#[dispatchable]` macro implementation.

use inflector::Inflector;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse::Result;

/// Transforms given name from `camelCase` to `snake_case` and adds `on_`
/// prefix.
fn to_handler_fn_name(name: &str) -> String {
    let mut snake_case = name.to_snake_case();
    snake_case.insert_str(0, "on_");
    snake_case
}

/// Generates the actual code for `#[dispatchable]` macro.
///
/// # Algorithm
///
/// 1. Generate dispatching `match`-arms for each `enum` variant.
/// 2. Generate trait methods signatures by transforming `enum` variant name
///    from `camelCase` to `snake_case` and add `on_` prefix.
/// 3. Generate trait `{enum_name}Handler` with generated methods from step 1.
/// 4. Generate method `dispatch_with()` with a dispatching generated on step 2.
pub fn derive(input: TokenStream) -> Result<TokenStream> {
    let mut output = input.clone();

    let item_enum: syn::ItemEnum = syn::parse(input)?;
    let enum_name = item_enum.ident.to_string();
    let enum_ident = item_enum.ident.clone();

    let dispatch_variants: Vec<_> = item_enum
        .variants
        .iter()
        .map(|v| {
            let enum_ident = item_enum.ident.clone();
            let variant_ident = v.ident.clone();
            let handler_fn_ident = syn::Ident::new(
                &to_handler_fn_name(&variant_ident.to_string()),
                Span::call_site(),
            );
            let fields: &Vec<_> = &v
                .fields
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    f.ident.clone().unwrap_or_else(|| {
                        syn::Ident::new(&format!("f{}", i), Span::call_site())
                    })
                })
                .collect();
            match v.fields {
                syn::Fields::Named(_) => quote! {
                    #enum_ident::#variant_ident {#(#fields),*} => {
                        handler.#handler_fn_ident(#(#fields),*)
                    },
                },
                syn::Fields::Unnamed(_) => quote! {
                    #enum_ident::#variant_ident(#(#fields),*) => {
                        handler.#handler_fn_ident((#(#fields),*))
                    },
                },
                syn::Fields::Unit => quote! {
                    #enum_ident::#variant_ident => handler.#handler_fn_ident(),
                },
            }
        })
        .collect();

    let handler_trait_ident = syn::Ident::new(
        &format!("{}Handler", item_enum.ident.to_string()),
        Span::call_site(),
    );
    let handler_trait_methods: Vec<_> = item_enum
        .variants
        .iter()
        .map(|v| {
            let fn_name_ident = syn::Ident::new(
                &to_handler_fn_name(&v.ident.to_string()),
                Span::call_site(),
            );
            let args = match v.fields {
                syn::Fields::Named(ref fields) => {
                    let args: Vec<_> = fields
                        .named
                        .iter()
                        .map(|f| {
                            let ident = f.ident.as_ref().unwrap();
                            let ty = &f.ty;
                            quote! { #ident: #ty }
                        })
                        .collect();
                    quote! { #(#args),* }
                }
                syn::Fields::Unnamed(ref fields) => {
                    let args: Vec<_> =
                        fields.unnamed.iter().map(|f| f.ty.clone()).collect();
                    quote! { data: (#(#args),*) }
                }
                _ => quote! {},
            };
            let doc = format!(
                "Handles [`{0}::{1}`] variant of [`{0}`].",
                enum_name,
                v.ident.to_string(),
            );
            quote! {
                #[doc = #doc]
                fn #fn_name_ident(&mut self, #args);
            }
        })
        .collect();

    let trait_doc = format!(
        "Handler of [`{0}`] variants.\n\nUsing [`{0}::dispatch_with`] method \
         dispatches [`{0}`] variants to appropriate methods of this trait.",
        enum_name
    );
    let method_doc =
        format!("Dispatches [`{0}`] with given [`{0}Handler`].", enum_name);
    let event_dispatch_impl = quote! {
        #[automatically_derived]
        #[doc = #trait_doc]
        pub trait #handler_trait_ident {
            #(#handler_trait_methods)*
        }

        #[automatically_derived]
        impl #enum_ident {
            #[doc = #method_doc]
            pub fn dispatch_with<T: #handler_trait_ident>(
                self, handler: &mut T,
            ) {
                match self {
                    #(#dispatch_variants)*
                }
            }
        }
    };

    output.extend(TokenStream::from(event_dispatch_impl));
    Ok(output)
}

#[cfg(test)]
mod to_handler_fn_name_spec {
    use super::to_handler_fn_name;

    #[test]
    fn converts_name_from_camel_case_to_snake_case() {
        for (name, expected) in &[
            ("SomeTestTrait", "on_some_test_trait"),
            ("RPCConnection", "on_rpc_connection"),
            ("RConnection", "on_r_connection"),
            ("RTCPeerConnection", "on_rtc_peer_connection"),
            ("testString", "on_test_string"),
            ("testtest", "on_testtest"),
            ("Some", "on_some"),
            ("S", "on_s"),
            ("s", "on_s"),
            ("ASDF", "on_asdf"),
        ] {
            assert_eq!(to_handler_fn_name(name), *expected);
        }
    }
}
