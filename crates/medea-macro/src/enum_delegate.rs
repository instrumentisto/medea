//! `#[enum_delegate]` macro implementation.

use std::iter;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Error, Result},
    spanned::Spanned as _,
};

/// Generates the actual code for `#[enum_delegate]` macro.
///
/// # Algorithm
///
/// 1. Check that input `enum` is not empty.
/// 2. Add `{}` to the macro argument (given function declaration).
/// 3. Check that delegation function is not static (presence of
///    `[&][&mut] self` arguments).
/// 4. Collect all the delegation function arguments.
/// 5. Generate wrapper-function with dispatching delegation function call to
///    all the `enum` variants.
///
/// # Limitations
///
/// - Cannot delegate static methods.
pub fn derive(args: &TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut output = input.clone();
    let inp: syn::DeriveInput = syn::parse(input)?;

    let mut enum_name_iter = iter::repeat(inp.ident.clone());
    let enum_name = enum_name_iter.next().unwrap();

    let variants: Vec<syn::Ident> = match &inp.data {
        syn::Data::Enum(ref data) => {
            data.variants.iter().map(|c| c.ident.clone()).collect()
        }
        _ => {
            return Err(Error::new(
                inp.span(),
                "This macro can be used on enums only",
            ))
        }
    };
    if variants.is_empty() {
        return Err(Error::new(
            inp.span(),
            "This macro can be used on non-empty enums only",
        ));
    }

    let arg_function = format!("{} {{ }}", args.to_string());
    let mut function: syn::ItemFn = syn::parse_str(&arg_function)?;

    let selfs_count = function
        .sig
        .inputs
        .iter()
        .filter(|i| matches!(i, syn::FnArg::Receiver(_)))
        .count();
    if selfs_count == 0 {
        return Err(Error::new(
            function.span(),
            "This macro can be used for non-static methods only",
        ));
    }

    let function_ident = iter::repeat(function.sig.ident.clone());
    let function_args = iter::repeat(
        function
            .sig
            .inputs
            .clone()
            .into_iter()
            .filter_map(|i| match i {
                syn::FnArg::Typed(c) => Some(c.pat),
                syn::FnArg::Receiver { .. } => None,
            })
            .collect::<Vec<_>>(),
    );

    let enum_output = quote! {
        #(#enum_name_iter::#variants(inner) => {
            inner.#function_ident(#(#function_args,)*)
        },)*
    };

    // This used for easy **body** generation by quote.
    let generated_fn: syn::ItemFn = syn::parse(
        quote! {
            pub fn a(&self) {
                match self {
                    #enum_output
                }
            }
        }
        .into(),
    )
    .unwrap();
    function.block = generated_fn.block;

    let impl_output = quote! {
        #[automatically_derived]
        impl #enum_name {
            #[inline]
            #function
        }
    };

    let impl_output: TokenStream = impl_output.into();
    output.extend(impl_output);

    Ok(output)
}
