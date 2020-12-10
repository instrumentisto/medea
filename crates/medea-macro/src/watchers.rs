use proc_macro::TokenStream;
use quote::quote;
use syn::{ExprMethodCall, ImplItem, ItemImpl};

mod kw {
    syn::custom_keyword!(watch);
}

pub fn expand(impl_item: ItemImpl) -> TokenStream {
    let mut out = impl_item.clone();

    let mut watchers = Vec::new();
    for item in impl_item.items {
        match item {
            ImplItem::Method(method) => {
                let attr = method
                    .attrs
                    .into_iter()
                    .find(|attr| *attr.path.get_ident().unwrap() == "watch")
                    .unwrap();
                let out: ExprMethodCall = attr.parse_args().unwrap();

                watchers.push((method.sig.ident, out));
            }
            _ => continue,
        }
    }

    let (ident, method): (Vec<_>, Vec<_>) = watchers.into_iter().unzip();
    let output = syn::parse_quote! {
        pub fn spawn(&self) {
            #(self.spawn_observer(#method, Self::#ident);)*
        }
    };

    out.items.push(output);

    let out = quote! { #out };
    out.into()
}
