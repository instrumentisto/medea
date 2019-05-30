//! `#[dispatchable]` macro implementation.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse::Result;

/// Named variant of enum.
#[derive(Clone)]
struct NamedVariant {
    /// Identifier of enum variant.
    ident: syn::Ident,

    /// Fields of enum variant.
    fields: Vec<NamedVariantField>,
}

/// Field of [`NamedVariant`].
#[derive(Clone)]
struct NamedVariantField {
    /// Identifier of enum field.
    ident: syn::Ident,

    /// Type of enum field.
    ty: syn::Type,
}

/// Field of [`UnnamedVariant`].
#[derive(Clone)]
struct UnnamedVariantField {
    /// Type of enum field.
    ty: syn::Type,
}

/// Unnamed variant of enum.
#[derive(Clone)]
struct UnnamedVariant {
    /// Identifier of enum variant.
    ident: syn::Ident,

    /// Fields of enum variant.
    fields: Vec<UnnamedVariantField>,
}

/// Type of enum variant.
///
/// [`VariantType::Named`] - `SomeEnum::NamedVariant { field: i32 }`
///
/// [`VariantType::Named`] - `SomeEnum::EmptyVariant`
///
/// [`VariantType::Unnamed`] - `SomeEnum::UnnamedVariant(u32)`
#[derive(Clone)]
enum VariantType {
    Named(NamedVariant),
    Unnamed(UnnamedVariant),
}

/// Transform function name from `snake_case` to `camelCase` and add "on_"
/// prefix.
fn to_handler_fn_name(event: &str) -> String {
    let mut snake_case = String::new();
    snake_case.push_str("on_");
    let mut prev_ch = '\0';

    for ch in event.chars() {
        if ch.is_uppercase() && !prev_ch.is_uppercase() && (prev_ch != '\0') {
            snake_case.push('_');
        }
        snake_case.push_str(&ch.to_lowercase().to_string());
        prev_ch = ch;
    }

    snake_case
}

/// Parse all [`VariantType`]s of provided enum.
fn parse_match_variants(enum_input: syn::ItemEnum) -> Vec<VariantType> {
    let mut match_variants = Vec::new();

    for v in enum_input.variants {
        let variant_ident = v.ident;

        match v.fields {
            syn::Fields::Named(f) => {
                let fields = f
                    .named
                    .into_iter()
                    .map(|f| NamedVariantField {
                        ident: f.ident.unwrap(),
                        ty: f.ty,
                    })
                    .collect();
                match_variants.push(VariantType::Named(NamedVariant {
                    ident: variant_ident,
                    fields,
                }));
            }
            syn::Fields::Unit => {
                match_variants.push(VariantType::Named(NamedVariant {
                    ident: variant_ident,
                    fields: Vec::new(),
                }));
            }
            syn::Fields::Unnamed(f) => {
                let fields = f
                    .unnamed
                    .into_iter()
                    .map(|f| UnnamedVariantField { ty: f.ty })
                    .collect();

                match_variants.push(VariantType::Unnamed(UnnamedVariant {
                    ident: variant_ident,
                    fields,
                }));
            }
        };
    }

    match_variants
}

/// Generates the actual code for `#[dispatchable]` macro.
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
    let mut output = input.clone();

    let item_enum: syn::ItemEnum = syn::parse(input)?;
    let enum_ident = item_enum.ident.clone();

    let variants = parse_match_variants(item_enum);
    let trait_variants = variants.clone();

    let variants = variants.into_iter().map(|variant_type| {
        let enum_ident = enum_ident.clone();

        match variant_type {
            VariantType::Named(v) => {
                let fields = v.fields;
                let variant_ident = v.ident;

                let fields_names = fields.into_iter().map(|f| f.ident);

                let handler_fn_name =
                    to_handler_fn_name(&variant_ident.to_string());
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
            }

            VariantType::Unnamed(v) => {
                let fields = v.fields;
                let variant_ident = v.ident;

                let tuple_fields_names =
                    fields.iter().enumerate().map(|(i, _)| {
                        syn::Ident::new(
                            &format!("field_{}", i),
                            Span::call_site(),
                        )
                    });

                let handler_fn_name =
                    to_handler_fn_name(&variant_ident.to_string());
                let handler_fn_ident: syn::Ident =
                    syn::parse_str(&handler_fn_name).unwrap();

                let fields_output = quote! {
                    #(#tuple_fields_names,)*
                };

                let match_body = quote! {
                    #enum_ident::#variant_ident (#fields_output) => {
                        handler.#handler_fn_ident((#fields_output))
                    },
                };

                match_body
            }
        }
    });

    let trait_functions =
        trait_variants
            .into_iter()
            .map(|variant_type| match variant_type {
                VariantType::Named(v) => {
                    let fn_name: syn::Ident = syn::parse_str(
                        &to_handler_fn_name(&v.ident.to_string()),
                    )
                    .unwrap();
                    let fn_args = v.fields.into_iter().map(|f| {
                        let ident = f.ident;
                        let tt = f.ty;
                        quote! {
                            #ident: #tt
                        }
                    });
                    let fn_out = quote! {
                        /// Name of this function generated by pattern
                        /// `on_{enum_variant_name}`.
                        fn #fn_name(&mut self, #(#fn_args,)*);
                    };

                    fn_out
                }

                VariantType::Unnamed(v) => {
                    let fn_name: syn::Ident = syn::parse_str(
                        &to_handler_fn_name(&v.ident.to_string()),
                    )
                    .unwrap();
                    let fn_tuple_inner = v.fields.into_iter().map(|f| f.ty);
                    let fn_out = quote! {
                        fn #fn_name(&mut self, data: (#(#fn_tuple_inner,)*));
                    };

                    fn_out
                }
            });

    let handler_trait_ident: syn::Ident =
        syn::parse_str(&format!("{}Handler", enum_ident.to_string()))?;

    let event_dispatch_impl = quote! {
        /// This trait is generated by `#[dispatchable]` macro.
        /// Name of this trait generated by pattern `{enum_name}Handler`.
        #[automatically_derived]
        pub trait #handler_trait_ident {
            #(#trait_functions)*
        }

        #[automatically_derived]
        impl #enum_ident {
            /// This function generated by `#[dispatchable]` macro.
            /// Call this function when you want to dispatch event.
            #[inline]
            pub fn dispatch<T: #handler_trait_ident>(self, handler: &mut T) {
                match self {
                    #(#variants)*
                }
            }
        }
    };

    output.extend(TokenStream::from(event_dispatch_impl));

    Ok(output)
}
