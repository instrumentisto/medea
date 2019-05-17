//! state_machine_shared_fn_accessor macro implementation.

use proc_macro::TokenStream;
use quote::quote;
use syn::export::Span;

pub fn derive(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut output = input.clone();
    let inp: syn::DeriveInput =
        syn::parse(input).expect("failed to parse input");

    let mut enum_name_iter = std::iter::repeat(inp.ident);
    let enum_name = enum_name_iter.next().unwrap();

    let variants: Vec<syn::Ident> = match inp.data {
        syn::Data::Enum(data) => {
            data.variants.into_iter().map(|c| c.ident).collect()
        }
        _ => panic!("This macro should be used only with enums!"),
    };

    let attribute_arguments = args.to_string();
    let mut attribute_arguments =
        attribute_arguments.split("->").map(|i| i.trim());

    let function = attribute_arguments.next().expect("Not provided function!");
    let function = syn::Ident::new(&function, Span::call_site());
    let function_iter = std::iter::repeat(function.clone());

    let result = attribute_arguments.next().expect("Not provided result!");
    let result: syn::Path = syn::parse_str(&result).unwrap();

    let enum_output = quote! {
        #(#enum_name_iter::#variants(inner) => inner.#function_iter(),)*
    };
    let impl_output = quote! {
        impl #enum_name {
            pub fn #function(&self) -> #result {
                match self {
                    #enum_output
                }
            }
        }
    };

    let impl_output: TokenStream = impl_output.into();
    output.extend(impl_output);

    output
}
