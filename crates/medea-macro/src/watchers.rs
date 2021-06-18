//! `#[watchers]` macro implementation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Error, Result},
    ExprMethodCall, ImplItem, ItemImpl,
};

/// Generates the actual code for `#[watchers]` macro.
///
/// # Algorithm
///
/// 1. Collects all methods with a `#[watch(...)]` attribute.
///
/// 2. Removes `#[watch(...)]` attributes from the found methods.
///
/// 3. Generates `WatchersSpawner::spawn()` code for the found methods.
///
/// 4. Generates `ComponentState` implementation with all the generated
///    `WatchersSpawner::spawn()` method calls.
///
/// 5. Appends the generated `ComponentState` implementation to the input.
pub fn expand(mut input: ItemImpl) -> Result<TokenStream> {
    let component_ty = input.self_ty.clone();

    #[allow(clippy::manual_filter_map)]
    let watchers = input
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
                s.spawn(#stream_expr, #component_ty::#watcher_ident);
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let component = quote! {
        <#component_ty as crate::utils::component::ComponentTypes>
    };

    Ok(quote! {
        #input

        impl crate::utils::component::ComponentState<
            #component::Obj,
        > for #component::State {
            fn spawn_watchers(
                &self,
                s: &mut crate::utils::component::WatchersSpawner<
                    Self,
                    #component::Obj,
                >,
            ) {
                #( #watchers )*
            }
        }
    }
    .into())
}
