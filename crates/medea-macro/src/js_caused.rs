use proc_macro2::TokenStream;
use quote::quote;
use synstructure::{BindStyle, Structure};

/// Generates the actual code for `#[derive(JsCaused)]` macro.
///
/// # Algorithm
///
/// 1. Generate body for trait method `name()` as `enum` variant name as is.
/// 2. Generate body for trait method `js_cause()`:
///     - if `enum` variant contains associated error, returns this error;
///     - if `enum` variant contains `JsCaused`, invoke trait method;
///     - otherwise returns `None`.
/// 3. Generate implementation `JsCaused` trait for this `enum` with generated
/// methods from step 1 and 2.
#[allow(clippy::needless_pass_by_value)]
pub fn derive(mut s: Structure) -> TokenStream {
    let error_type = error_type(&s).expect("not found js_error attribute");

    let name_body = s.each_variant(|v| {
        let name = &v.ast().ident;
        quote!(stringify!(#name))
    });

    let cause_body = s.bind_with(|_| BindStyle::Move).each_variant(|v| {
        if let Some(js_error) =
            v.bindings().iter().find(|&bi| is_error(bi, &error_type))
        {
            quote!(return Some(#js_error))
        } else if let Some(js_caused) =
            v.bindings().iter().find(|&bi| is_caused(bi))
        {
            quote!(return #js_caused.js_cause())
        } else {
            quote!(return None)
        }
    });

    let js_caused = s.gen_impl(quote! {
        gen impl JsCaused for @Self {
            type Error = #error_type;

            fn name(&self) -> &'static str {
                match self { #name_body }
            }

            fn js_cause(self) -> Option<Self::Error> {
                match self { #cause_body }
            }
        }
    });

    quote! { #js_caused }
}

/// Parse and returns argument of `js_error` attribute.
fn error_type(s: &synstructure::Structure) -> Option<syn::Path> {
    let mut error_type = None;
    for attr in &s.ast().attrs {
        if let Ok(meta) = attr.parse_meta() {
            if meta.path().is_ident("js_error") {
                if error_type.is_some() {
                    panic!("Cannot have two `js_error` attributes");
                }
                match meta {
                    syn::Meta::List(types) => {
                        if types.nested.len() != 1 {
                            panic!(
                                "Expected at only one argument to js_error \
                                 attribute"
                            );
                        }
                        error_type = match &types.nested[0] {
                            syn::NestedMeta::Meta(syn::Meta::Path(path)) => {
                                Some(path.clone())
                            }
                            _ => panic!(
                                "Invalid argument for js_error attribute"
                            ),
                        };
                    }
                    _ => panic!(
                        "Expected attribute like `#[js_error(path::to::Error)]"
                    ),
                }
            }
        }
    }
    error_type
}

/// Checks what enum variant contains type error from `js_error` attribute.
fn is_error(bi: &synstructure::BindingInfo, err: &syn::Path) -> bool {
    match &bi.ast().ty {
        syn::Type::Path(syn::TypePath { qself: None, path }) => path == err,
        _ => false,
    }
}

/// Checks what enum variant has attribute `js_cause`.
fn is_caused(bi: &synstructure::BindingInfo) -> bool {
    let mut found_cause = false;
    for attr in &bi.ast().attrs {
        if let Ok(meta) = attr.parse_meta() {
            if meta.path().is_ident("js_cause") {
                if found_cause {
                    panic!("Cannot have two `js_cause` attributes");
                }
                found_cause = true;
            }
        }
    }
    found_cause
}
