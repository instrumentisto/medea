//! EventDispatcher macro implementation.

use proc_macro::TokenStream;

#[derive(Clone)]
struct MatchVariant {
    ident: syn::Ident,
    fields: Vec<MatchVariantField>,
}

#[derive(Clone)]
struct MatchVariantField {
    ident: syn::Ident,
    ty: syn::Type,
}

// Do not use it with names like SendRDP, ReceiveRDP, HTTP!
// For this names this function generate names like
// on_send_r_d_p, on_receive_r_d_p, on_h_t_t_p!
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

fn parse_match_variants(enum_input: syn::ItemEnum) -> Vec<MatchVariant> {
    let variants = enum_input.variants.into_iter()
        .map(|v| {
            let variant_ident = v.ident;

            let fields = match v.fields {
                syn::Fields::Named(f) => {
                    f.named.into_iter().map(|f| {
                        MatchVariantField {
                            ident: f.ident.unwrap(), // This is only named field macro
                            ty: f.ty
                        }
                    }).collect::<Vec<MatchVariantField>>()
                },
                _ => unimplemented!()
            };

            MatchVariant {
                ident: variant_ident,
                fields,
            }
        }).collect::<Vec<MatchVariant>>();

    variants
}

pub fn derive(input: TokenStream) -> TokenStream {
    let item_enum: syn::ItemEnum = syn::parse(input).unwrap();
    let enum_ident = item_enum.ident.clone();

    let variants = parse_match_variants(item_enum);
    let trait_variants = variants.clone();

    let variants = variants.into_iter().map(|v| {
        let enum_ident = enum_ident.clone();
        let fields = v.fields;
        let variant_ident = v.ident;
        let fields = fields.into_iter()
            .map(|f| {
                f.ident
            });
        let handler_fn_name = to_handler_fn_name(&variant_ident.to_string());
        let handler_fn_ident: syn::Ident = syn::parse_str(&handler_fn_name).unwrap();

        let fields_output = quote! {
            #(#fields,)*
        };

        let match_body = quote! {
            #enum_ident::#variant_ident {#fields_output} => {handler.#handler_fn_ident(#fields_output)},
        };

        match_body
    });

    let trait_functions = trait_variants.into_iter().map(|v| {
        let fn_name: syn::Ident = syn::parse_str(&to_handler_fn_name(&v.ident.to_string())).unwrap();
        let field_types = v.fields.into_iter()
            .map(|f| {
                let ident = f.ident;
                let tt = f.ty;
                quote! {
                    #ident: #tt
                }
            });
        let fn_out = quote! {
            fn #fn_name(&self, #(#field_types,)*);
        };

        fn_out
    });

    let handler_trait_ident: syn::Ident = syn::parse_str(&format!("{}Handler", enum_ident.to_string())).unwrap();

    let event_dispatch_impl = quote! {
        pub trait #handler_trait_ident {
            #(#trait_functions)*
        }

        impl #enum_ident {
            pub fn dispatch<T: #handler_trait_ident>(self, handler: &T) {
                match self {
                    #(#variants)*
                }
            }
        }
    };


    event_dispatch_impl.into()
}
