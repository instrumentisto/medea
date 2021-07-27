use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Error, Result};
use synstructure::{BindStyle, Structure};

/// Generates the actual code for `#[derive(JsCaused)]` macro.
///
/// # Algorithm
///
/// 1. Generate body for trait method `name()` as enum variant name "as is".
/// 2. Generate body for trait method `js_cause()`:
///     - if `enum` variant contains associated error, returns this error;
///     - if `enum` variant contains `JsCaused`, invoke its trait method;
///     - otherwise returns `None`.
/// 3. Generate implementation of `JsCaused` trait for this enum with generated
///    methods from step 1 and 2.
#[allow(clippy::needless_pass_by_value)]
pub fn derive(mut s: Structure) -> Result<TokenStream> {
    let error_type = error_type(&s)?;

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
        #[automatically_derived]
        gen impl JsCaused for @Self {
            type Error = #error_type;

            fn js_cause(self) -> Option<Self::Error> {
                match self { #cause_body }
            }
        }
    });

    Ok(quote!(#js_caused))
}

/// Parse and returns argument of `#[js(error = "path::to::Error"))]` attribute.
/// If no such attribute exists the defaults to `JsError`.
fn error_type(s: &synstructure::Structure) -> Result<syn::Path> {
    let mut error_type = None;
    for attr in &s.ast().attrs {
        if let Ok(meta) = attr.parse_meta() {
            if meta.path().is_ident("js") {
                if error_type.is_some() {
                    return Err(Error::new_spanned(
                        meta,
                        "Cannot have two #[js(...)] attributes",
                    ));
                }
                if let syn::Meta::List(list) = meta {
                    if list.nested.is_empty() {
                        return Err(Error::new_spanned(
                            list,
                            "Expected at least one argument to #[js(...)] \
                             attribute",
                        ));
                    }
                    error_type =
                        match &list.nested[0] {
                            syn::NestedMeta::Meta(syn::Meta::NameValue(nv))
                                if nv.path.is_ident("error") =>
                            {
                                if let syn::MetaNameValue {
                                    lit: syn::Lit::Str(lit_str),
                                    ..
                                } = nv
                                {
                                    Some(lit_str.parse_with(
                                        syn::Path::parse_mod_style,
                                    )?)
                                } else {
                                    return Err(Error::new_spanned(
                                        nv,
                                        "Expected `path::to::error`",
                                    ));
                                }
                            }
                            _ => {
                                return Err(Error::new_spanned(
                                    list,
                                    "Expected attribute like #[js(error = \
                                     \"path::to::error\")]",
                                ));
                            }
                        };
                } else {
                    return Err(Error::new_spanned(
                        meta,
                        "#[js] attribute must take a list in parentheses",
                    ));
                };
            }
        }
    }
    match error_type {
        Some(path) => Ok(path),
        None => syn::LitStr::new("JsError", Span::call_site()).parse(),
    }
}

/// Checks that enum variant has `#[js(cause)]` attribute.
fn is_caused(bi: &synstructure::BindingInfo) -> bool {
    let mut found_cause = false;
    for attr in &bi.ast().attrs {
        if let Ok(meta) = attr.parse_meta() {
            if meta.path().is_ident("js") {
                if let syn::Meta::List(ref list) = meta {
                    if let Some(syn::NestedMeta::Meta(syn::Meta::Path(
                        ref path,
                    ))) = list.nested.first()
                    {
                        if path.is_ident("cause") {
                            if found_cause {
                                panic!("Cannot have two cause attributes");
                            }
                            found_cause = true;
                        }
                    }
                }
            }
        }
    }
    found_cause
}

/// Checks that enum variant contains JS error.
fn is_error(bi: &synstructure::BindingInfo, err: &syn::Path) -> bool {
    match &bi.ast().ty {
        syn::Type::Path(syn::TypePath { qself: None, path }) => path == err,
        _ => false,
    }
}
