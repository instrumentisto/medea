//! `#[watchers]` and `#[watch(...)]` macros implementation.

use proc_macro::TokenStream;
use quote::{quote, ToTokens as _};
use syn::{
    parse::{Error, Result},
    ExprMethodCall, ImplItem, ItemImpl,
};

/// Generates the actual code for `#[watchers]` macro.
///
/// # Algorithm
///
/// 1. Collects all methods with a `#[watch(...)]` macro.
///
/// 2. Generates `spawn_watcher` code for the found methods.
///
/// 3. Generates `spawn` method with all generated `spawn_watcher` method calls.
///
/// 4. Appends generated `spawn` method to the input [`ItemImpl`].
pub fn expand(mut input: ItemImpl) -> Result<TokenStream> {
    #[allow(clippy::filter_map)]
    let watchers: Vec<_> = input
        .items
        .iter_mut()
        .filter_map(|i| {
            if let ImplItem::Method(m) = i {
                Some(m)
            } else {
                None
            }
        })
        .map(|method| {
            let mut watch_attr_index = None;
            let stream_expr = method
                .attrs
                .iter()
                .enumerate()
                .find_map(|(i, attr)| {
                    if attr.path.get_ident().map_or(false, |p| *p == "watch") {
                        watch_attr_index = Some(i);
                        Some(attr)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    Error::new(
                        method.sig.ident.span(),
                        "Method doesn't have '#[watch(...)]' macro",
                    )
                })?
                .parse_args::<ExprMethodCall>()?;
            if let Some(index) = watch_attr_index {
                method.attrs.remove(index);
            }
            let watcher_ident = &method.sig.ident;

            Ok(quote! {
                self.spawn_watcher(#stream_expr, Self::#watcher_ident);
            })
        })
        .collect::<Result<_>>()?;

    let mut output = input;
    output.items.push(syn::parse_quote! {
        /// Spawns all watchers of this [`Component`].
        #[automatically_derived]
        pub fn spawn(&self) {
            #( #watchers )*
        }
    });

    Ok(output.to_token_stream().into())
}
