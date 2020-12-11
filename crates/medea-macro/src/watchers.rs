use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Error, Result},
    ExprMethodCall, ImplItem, ItemImpl,
};

pub fn expand(input: ItemImpl) -> Result<TokenStream> {
    let watchers: Vec<_> = input
        .items
        .iter()
        .filter_map(|i| {
            if let ImplItem::Method(m) = i {
                Some(m)
            } else {
                None
            }
        })
        .map(|method| {
            let stream_expr: ExprMethodCall = method
                .attrs
                .iter()
                .find(|attr| {
                    attr.path
                        .get_ident()
                        .map(|p| *p == "watch")
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    Error::new(
                        method.sig.ident.span(),
                        "Method doesn't have '#[watch(...)]' macro",
                    )
                })?
                .parse_args()?;
            let watcher_ident = &method.sig.ident;

            Ok(quote! {
                self.spawn_watcher(#stream_expr, Self::#watcher_ident);
            })
        })
        .collect::<Result<_>>()?;

    let mut output = input;
    output.items.push(syn::parse_quote! {
        pub fn spawn(&self) {
            #(#watchers)*
        }
    });

    Ok((quote! { #output }).into())
}
