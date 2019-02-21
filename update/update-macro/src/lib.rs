#![recursion_limit = "128"]

extern crate proc_macro;
extern crate quote;
extern crate syn;
extern crate update_trait;

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(Update)]
pub fn update_derive(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemStruct);
    let name = &input.ident;
    let idents: Vec<_> = input
        .fields
        .iter()
        .filter_map(|field| field.ident.clone())
        .collect();
    let updates = idents
        .clone()
        .into_iter()
        .map(|ident| quote! { stringify!(#ident) => self.#ident.update(key, value) });
    let clears = idents
        .iter()
        .map(|ident| quote! { stringify!(#ident) => self.#ident.clear(key) });

    let result = quote! {
        impl UpdateTrait for #name {
            fn update<I: Iterator<Item=String>>(&mut self, mut key: I, value: String) -> Result<(), &'static str> {
                match key.next().ok_or("Update failed - not declared for struct")?.as_str() {
                    #(#updates,)*
                    _ => Err("Update failed - unknown struct field"),
                }
            }

            fn clear<I: Iterator<Item=String>>(&mut self, mut key: I) -> Result<(), &'static str> {
                match key.next().ok_or("Clear failed - not declared for struct")?.as_str() {
                    #(#clears,)*
                    _ => Err("Close failed - unknown struct field"),
                }
            }
        }
    };
    result.into()
}
