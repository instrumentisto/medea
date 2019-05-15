#[macro_use]
extern crate quote;
extern crate proc_macro;
extern crate syn;

use proc_macro::TokenStream;
use syn::export::Span;

/// This macro should be used for creating shared state function's accessor.
/// ## How to use:
///
/// ```
/// #[state_machine_shared_fn_accessor(/*SHARED_STATE_METHOD*/ -> /*RETURN_TYPE_OF_THIS_METHOD*/)]
/// enum SomeStateMachine {/*...*/}
/// ```
///
/// ## Example:
/// ```
/// struct SomeState;
/// struct AnotherState;
///
/// struct Context {
///     some_value: i32,
/// }
///
/// struct Peer<S> {
///     context: Context,
///     state: S,
/// }
///
/// impl<T> Peer<T> {
///     pub fn some_value(&self) -> i32 {
///         self.context.some_value
///     }
/// }
///
/// #[state_machine_shared_fn_accessor(some_value -> i32)]
/// enum PeerStateMachine {
///     SomeState(Peer<SomeState>),
///     AnotherState(Peer<AnotherState>),
/// }
///
/// fn main() {
///     let peer = PeerStateMachine::SomeState(Peer {
///         context: Context {
///             some_value: 10,
///         },
///         state: SomeState,
///     });
///
///     peer.some_value() // -> 10
/// }
/// ```
#[proc_macro_attribute]
pub fn state_machine_shared_fn_accessor(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut output = input.clone();
    let inp: syn::DeriveInput = syn::parse(input).expect("failed to parse input");

    let mut enum_name_iter = std::iter::repeat(inp.ident);
    let enum_name = enum_name_iter.next().unwrap();

    let variants: Vec<syn::Ident> = match inp.data {
        syn::Data::Enum(data) => data.variants.into_iter().map(|c| c.ident).collect(),
        _ => panic!("This macro should be used only with enums!"),
    };

    let attribute_arguments = args.to_string();
    let mut attribute_arguments = attribute_arguments.split("->").map(|i| i.trim());

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
