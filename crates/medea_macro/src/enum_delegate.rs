//! `#[enum_delegate]` macro implementation.

use std::iter;

use proc_macro::TokenStream;
use quote::quote;

/// Generates the actual code for `#[enum_delegate]` macro.
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

    // This is for easy parsing function declaration by default syn parser.
    let arg_function = format!("{} {{ }}", args.to_string());
    let mut function: syn::ItemFn = syn::parse_str(&arg_function).unwrap();
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
