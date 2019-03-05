#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result as ParseResult};
use syn::{Ident, Token};

enum Item {
    Struct(syn::ItemStruct),
    Enum(syn::ItemEnum),
}

impl Parse for Item {
    fn parse(input: ParseStream) -> ParseResult<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![struct]) {
            input.parse().map(Item::Struct)
        } else if lookahead.peek(Token![enum]) {
            input.parse().map(Item::Enum)
        } else {
            Err(lookahead.error())
        }
    }
}

fn impl_set_for_enum(
    name: Ident,
    variants: Vec<syn::Variant>,
) -> impl Iterator<Item = proc_macro2::TokenStream> {
    variants.into_iter().map(move |variant| {
        let id = variant.ident;

        match variant.fields {
            syn::Fields::Named(ref fields) => {
                let cases = fields.named
                    .iter()
                    .map(|x| x.to_owned().ident.unwrap())
                    .map(|ident| quote! { stringify!(#ident) => #ident.set(key, value) });
                let fields = fields.named.iter().map(|x| x.ident.to_owned().unwrap());

                quote! {
                    stringify!(#id) => {
                        if let #name::#id { #(#fields),* } = self {
                            match key.next().ok_or("Clear failed - not declared for struct")?.as_str() {
                                #(#cases,)*
                                _ => Err("Close failed - unknown struct field"),
                            }
                        } else {
                            unreachable!()
                        }
                    }
                }
            },
            syn::Fields::Unnamed(ref fields) => {
                let blanks = fields.unnamed.iter().skip(1).map(|_| quote!(_));

                quote! {
                    stringify!(#id) => {
                        if let #name::#id( x, #(#blanks,)* ) = self {
                            x.set(key, value)
                        } else {
                            unreachable!()
                        }
                    }
                }
            }
            syn::Fields::Unit => {
                quote! { stringify!(#id) => {
                    if key.next() == None {
                        *self = #name::#id;
                        Ok(())
                    } else {
                        return Err("Remove failed - too long path")
                    }
                }}
            },
        }
    })
}

fn impl_remove_for_enum(
    name: Ident,
    variants: Vec<syn::Variant>,
) -> impl Iterator<Item = proc_macro2::TokenStream> {
    variants.into_iter().map(move |variant| {
        let id = variant.ident;

        match variant.fields {
            syn::Fields::Named(ref fields) => {
                let cases = fields.named
                    .iter()
                    .map(|x| x.to_owned().ident.unwrap())
                    .map(|ident| quote! { stringify!(#ident) => #ident.remove(key) });
                let fields = fields.named.iter().map(|x| x.ident.to_owned().unwrap());

                quote! {
                    stringify!(#id) => {
                        if let #name::#id { #(#fields),* } = self {
                            match key.next().ok_or("Clear failed - not declared for struct")?.as_str() {
                                #(#cases,)*
                                _ => Err("Close failed - unknown struct field"),
                            }
                        } else {
                            unreachable!()
                        }
                    }
                }
            },
            syn::Fields::Unnamed(ref fields) => {
                let blanks = fields.unnamed.iter().skip(1).map(|_| quote!(_));

                quote! {
                    stringify!(#id) => {
                        if let #name::#id( x, #(#blanks,)* ) = self {
                            x.remove(key)
                        } else {
                            unreachable!()
                        }
                    }
                }
            }
            syn::Fields::Unit => {
                quote! { stringify!(#id) => {
                    Err("Remove failed - unit struct on path")
                }}
            },
        }
    })
}

fn macro_for_enum(input: syn::ItemEnum) -> TokenStream {
    let fields: Vec<_> = input.variants.into_iter().collect();

    let name = input.ident.to_owned();
    let removes = impl_remove_for_enum(name.clone(), fields.clone());
    let sets = impl_set_for_enum(name.clone(), fields);

    let result = quote! {
        impl UpdateTrait for #name {
            fn remove<I: Iterator<Item=String>>(&mut self, mut key: I) -> Result<(), &'static str> {
                match key.next().ok_or("Too long path")?.as_ref() {
                    #(#removes,)*
                    _ => Err("Close failed - unknown enum field"),
                }
            }

            fn set<I: Iterator<Item=String>>(&mut self, mut key: I, value: String) -> Result<(), &'static str> {
                match key.next().ok_or("Too long path")?.as_ref() {
                    #(#sets,)*
                    _ => Err("Update failed - unknown enum field"),
                }
            }
        }
    };

    result.into()
}

fn impl_for_struct(idents: Vec<Ident>) -> impl quote::ToTokens {
    let removes = idents
        .iter()
        .map(|ident| quote! { stringify!(#ident) => self.#ident.remove(key) });

    let sets = idents
        .iter()
        .map(|ident| quote! { stringify!(#ident) => self.#ident.set(key, value) });

    quote! {
        fn remove<I: Iterator<Item=String>>(&mut self, mut key: I) -> Result<(), &'static str> {
            match key.next().ok_or("Clear failed - not declared for struct")?.as_str() {
                #(#removes,)*
                _ => Err("Close failed - unknown struct field"),
            }
        }

        fn set<I: Iterator<Item=String>>(&mut self, mut key: I, value: String) -> Result<(), &'static str> {
            match key.next().ok_or("Update failed - not declared for struct")?.as_str() {
                #(#sets,)*
                _ => Err("Update failed - unknown struct field"),
            }
        }
    }
}

fn macro_for_struct(input: syn::ItemStruct) -> TokenStream {
    let idents: Vec<_> = input
        .fields
        .iter()
        .filter_map(|field| field.ident.to_owned())
        .collect();

    let implementation = impl_for_struct(idents);
    let name = &input.ident;

    let result = quote! {
        impl UpdateTrait for #name {
            #implementation
        }
    };

    result.into()
}

#[proc_macro_derive(Update)]
pub fn update_derive(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as Item);

    match input {
        Item::Struct(x) => macro_for_struct(x),
        Item::Enum(x) => macro_for_enum(x),
    }
}
