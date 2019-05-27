//! `#[enum_delegate]` macro implementation.

use std::iter;

use proc_macro::TokenStream;
use quote::quote;

/// Generates the actual code for `#[enum_delegate]` macro.
///
/// # Generation algorithm
/// 1) parse input enum
/// 2) check that enum is not empty
/// 3) add "{}" to the macro argument to get the full function declaration.
/// 4) parse this full function declaration
/// 5) check that function to delegate is not static by counting `[&][&mut] self`
///    arguments
/// 6) get all argument idents (except `[&][&mut] self`) from a function to be
///    delegated
/// 7) generate function with `match` of all variants that returns
///    result of delegated function call
/// 8) add this function to implementation of enum
pub fn derive(args: &TokenStream, input: TokenStream) -> TokenStream {
    let mut output = input.clone();
    let inp: syn::DeriveInput =
        syn::parse(input).expect("failed to parse input");

    let mut enum_name_iter = iter::repeat(inp.ident);
    let enum_name = enum_name_iter.next().unwrap();

    let variants: Vec<syn::Ident> = match inp.data {
        syn::Data::Enum(data) => {
            data.variants.into_iter().map(|c| c.ident).collect()
        }
        _ => panic!("This macro should be used only with enums!"),
    };

    if variants.is_empty() {
        panic!("You provided empty enum!")
    }

    // This is for easy parsing function declaration by default syn parser.
    let arg_function = format!("{} {{ }}", args.to_string());
    let mut function: syn::ItemFn = syn::parse_str(&arg_function).unwrap();

    let selfs_count = function
        .decl
        .inputs
        .iter()
        .filter(|i| match i {
            syn::FnArg::SelfValue(_) | syn::FnArg::SelfRef(_) => true,
            _ => false,
        })
        .count();
    if selfs_count == 0 {
        panic!("Static functions not supported!");
    }

    let function_ident = iter::repeat(function.ident.clone());
    // Iterator over captured function args
    let function_args =
        iter::repeat(function.decl.clone().inputs.into_iter().filter_map(
            |i| match i {
                syn::FnArg::Captured(c) => Some(c.pat),
                _ => None,
            },
        ));

    let enum_output = quote! {
        #(#enum_name_iter::#variants(inner) => {
            inner.#function_ident(#(#function_args)*)
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

    output
}
