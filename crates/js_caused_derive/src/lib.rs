use proc_macro2::TokenStream;
use quote::quote;
use synstructure::{decl_derive, Structure};

decl_derive!([JsCaused, attributes(js_caused, js_cause)] => js_caused_derive);

/// Generates the actual code for `#[derive(JsCaused)]` macro.
///
/// # Algorithm
///
/// 1. Generate body for trait method `name()` as `enum` variant name as is.
/// 2. Generate body for trait method `js_cause()`:
///     - if `enum` variant contains `JsError`, returns this error;
///     - if `enum` variant contains `JsCaused`, invoke trait method;
///     - otherwise returns `None`.
/// 3. Generate implementation `JsCaused` trait for this `enum` with generated
/// methods from step 1 and 2.
#[allow(clippy::needless_pass_by_value)]
fn js_caused_derive(s: Structure) -> TokenStream {
    let name_body = s.each_variant(|v| {
        let name = &v.ast().ident;
        quote!(stringify!(#name))
    });

    let cause_body = s.each_variant(|v| {
        if let Some(js_error) = v.bindings().iter().find(|&bi| is_js_error(bi))
        {
            quote!(return Some(#js_error.into()))
        } else if let Some(js_caused) =
            v.bindings().iter().find(|&bi| is_js_caused(bi))
        {
            quote!(return #js_caused.js_cause())
        } else {
            quote!(return None)
        }
    });

    let js_caused = s.gen_impl(quote! {
        gen impl js_caused::JsCaused for @Self {
            fn name(&self) -> &'static str {
                match *self { #name_body }
            }

            fn js_cause(&self) -> Option<js_sys::Error> {
                match *self { #cause_body }
            }
        }
    });

    quote! { #js_caused }
}

/// Checks what enum variant contains `JsError`.
fn is_js_error(bi: &synstructure::BindingInfo) -> bool {
    match bi.ast().ty {
        syn::Type::Path(syn::TypePath {
            qself: None,
            path:
                syn::Path {
                    segments: ref path, ..
                },
        }) => path
            .last()
            .map_or(false, |s| s.ident == "JsError" && s.arguments.is_empty()),
        _ => false,
    }
}

/// Checks what enum variant has attribute `js_cause` or `js_caused[js_cause]`.
fn is_js_caused(bi: &synstructure::BindingInfo) -> bool {
    let mut found_cause = false;
    for attr in &bi.ast().attrs {
        if let Ok(meta) = attr.parse_meta() {
            if meta.path().is_ident("js_cause") {
                if found_cause {
                    panic!("Cannot have two `js_cause` attributes");
                }
                found_cause = true;
            }
            if meta.path().is_ident("js_caused") {
                if let syn::Meta::List(ref list) = meta {
                    if let Some(ref pair) = list.nested.first() {
                        if let syn::NestedMeta::Meta(syn::Meta::Path(
                            ref path,
                        )) = *(*pair)
                        {
                            if path.is_ident("js_cause") {
                                if found_cause {
                                    panic!(
                                        "Cannot have two `js_cause` attributes"
                                    );
                                }
                                found_cause = true;
                            }
                        }
                    }
                }
            }
        }
    }
    found_cause
}
