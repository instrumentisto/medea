//! `#[derive(EventDispatcher)]` macro implementation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Error, Result},
    spanned::Spanned as _,
};

/// Variant of enum.
#[derive(Clone)]
struct MatchVariant {
    /// Identifier of enum variant.
    ident: syn::Ident,

    /// Fields of enum variant.
    fields: Vec<MatchVariantField>,
}

/// Field of match variant.
#[derive(Clone)]
struct MatchVariantField {
    /// Identifier of enum field.
    ident: syn::Ident,

    /// Type of enum field.
    ty: syn::Type,
}

/// Transform function name from `snake_case` to `camelCase` and add "on_"
/// prefix.
///
/// Do not use it with names like `SendRDP`, `ReceiveRDP`, `HTTP`!
/// For this names this function generate names like
/// `on_send_r_d_p`, `on_receive_r_d_p`, `on_h_t_t_p`!
fn to_handler_fn_name(event: &str) -> String {
    let mut snake_case = String::new();
    snake_case.push_str("on");
    for ch in event.chars() {
        if ch.is_uppercase() {
            snake_case.push('_');
        }
        snake_case.push_str(&ch.to_lowercase().to_string());
    }

    snake_case
}

/// Parse all [`MatchVariant`]s of provided enum.
/// Support only named enums.
///
/// Returns error if enum have unnamed variant.
fn parse_match_variants(
    enum_input: syn::ItemEnum,
) -> Result<Vec<MatchVariant>> {
    let mut match_variants = Vec::new();

    for v in enum_input.variants {
        let variant_ident = v.ident;

        let fields: Vec<MatchVariantField> = match v.fields {
            syn::Fields::Named(f) => f
                .named
                .into_iter()
                .map(|f| MatchVariantField {
                    ident: f.ident.unwrap(),
                    ty: f.ty,
                })
                .collect(),
            syn::Fields::Unit => Vec::new(),
            syn::Fields::Unnamed(f) => {
                return Err(Error::new(
                    f.span(),
                    "This macro not support unnamed enum variants!",
                ));
            }
        };

        let variant = MatchVariant {
            ident: variant_ident,
            fields,
        };

        match_variants.push(variant);
    }

    Ok(match_variants)
}

/// Generates the actual code for `#[derive(EventDispatcher)]` macro.
///
/// # Generation algorithm
/// 1. parse variants of enum
/// 2. for every variant it does the following:
/// 2.1. get all variant fields
/// 2.2. generate function name
/// 2.3. generate `match` for this variant that call function with all
///      enum variant fields as argument
/// 3. generate trait functions declaration by transformation function name
///    from `snake_case` to `camelCase` and add "on_" prefix.
/// 4. generate trait `{enum_name}Handler` with generated functions declarations
///    from step 3.
/// 5. generate function `dispatch<T: {enum_name}Handler>(self, handler: &T)`
///    with `match` that generated in step 2.3.
pub fn derive(input: TokenStream) -> Result<TokenStream> {
    let item_enum: syn::ItemEnum = syn::parse(input)?;
    let enum_ident = item_enum.ident.clone();

    let variants = parse_match_variants(item_enum)?;
    let trait_variants = variants.clone();

    let variants = variants.into_iter().map(|v| {
        let enum_ident = enum_ident.clone();
        let fields = v.fields;
        let variant_ident = v.ident;

        let fields_names = fields.into_iter().map(|f| f.ident);

        let handler_fn_name = to_handler_fn_name(&variant_ident.to_string());
        let handler_fn_ident: syn::Ident =
            syn::parse_str(&handler_fn_name).unwrap();

        let fields_output = quote! {
            #(#fields_names,)*
        };

        let match_body = quote! {
            #enum_ident::#variant_ident {#fields_output} => {
                handler.#handler_fn_ident(#fields_output)
            },
        };

        match_body
    });

    let trait_functions = trait_variants.into_iter().map(|v| {
        let fn_name: syn::Ident =
            syn::parse_str(&to_handler_fn_name(&v.ident.to_string())).unwrap();
        let fn_args = v.fields.into_iter().map(|f| {
            let ident = f.ident;
            let tt = f.ty;
            quote! {
                #ident: #tt
            }
        });
        let fn_out = quote! {
            fn #fn_name(&mut self, #(#fn_args,)*);
        };

        fn_out
    });

    let handler_trait_ident: syn::Ident =
        syn::parse_str(&format!("{}Handler", enum_ident.to_string()))?;

    let event_dispatch_impl = quote! {
        pub trait #handler_trait_ident {
            #(#trait_functions)*
        }

        impl #enum_ident {
            pub fn dispatch<T: #handler_trait_ident>(self, handler: &mut T) {
                match self {
                    #(#variants)*
                }
            }
        }
    };

    Ok(event_dispatch_impl.into())
}
