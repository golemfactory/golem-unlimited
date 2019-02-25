#![recursion_limit = "128"]

extern crate proc_macro;
extern crate quote;
extern crate syn;
extern crate update_trait;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result as ParseResult};
use syn::punctuated::Punctuated;
use syn::{braced, token, Field, Ident, Result, Token};

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

fn set_in_enum(idents: Vec<Ident>, name: &Ident) -> impl quote::ToTokens {
    let updates = idents.into_iter().map(|ident| {
        quote! { #name::#ident(x) => {
            if stringify!(#ident) != key.next().ok_or("Clear failed - not declared for enum")? {
                return Err("Set failed - no such option in enum")
            }

            x.update(key, value)
        }}
    });

    quote! {
        fn update<I: Iterator<Item=String>>(&mut self, mut key: I, value: String) -> Result<(), &'static str> {
            match self {
                #(#updates,)*
                _ => Err("Update failed - unknown enum field"),
            }
        }
    }
}

fn remove_in_enum(idents: Vec<Ident>, name: &Ident) -> impl quote::ToTokens {
    let clears = idents.iter().map(|ident| {
        quote! { #name::#ident(x) => {
            if stringify!(#ident) != key.next().ok_or("Clear failed - not declared for enum")? {
                return Err("Set failed - no such option in enum")
            }

            x.clear(key)
        }}
    });

    quote! {
        fn clear<I: Iterator<Item=String>>(&mut self, mut key: I) -> Result<(), &'static str> {
            match self {
                #(#clears,)*
                _ => Err("Close failed - unknown enum field"),
            }
        }
    }
}

fn update_enum(input: syn::ItemEnum) -> TokenStream {
    let idents: Vec<_> = input
        .variants
        .iter()
        .map(|field| field.ident.to_owned())
        .collect();

    let name = &input.ident;
    let set_quote = set_in_enum(idents.clone(), name);
    let remove_quote = remove_in_enum(idents, name);

    let result = quote! {
        impl UpdateTrait for #name {
            #set_quote
            #remove_quote
        }
    };

    result.into()
}

fn set_in_struct(idents: Vec<Ident>) -> impl quote::ToTokens {
    let updates = idents
        .into_iter()
        .map(|ident| quote! { stringify!(#ident) => self.#ident.update(key, value) });

    quote! {
        fn update<I: Iterator<Item=String>>(&mut self, mut key: I, value: String) -> Result<(), &'static str> {
            match key.next().ok_or("Update failed - not declared for struct")?.as_str() {
                #(#updates,)*
                _ => Err("Update failed - unknown struct field"),
            }
        }
    }
}

fn remove_in_struct(idents: Vec<Ident>) -> impl quote::ToTokens {
    let clears = idents
        .iter()
        .map(|ident| quote! { stringify!(#ident) => self.#ident.clear(key) });

    quote! {
        fn clear<I: Iterator<Item=String>>(&mut self, mut key: I) -> Result<(), &'static str> {
            match key.next().ok_or("Clear failed - not declared for struct")?.as_str() {
                #(#clears,)*
                _ => Err("Close failed - unknown struct field"),
            }
        }
    }
}

fn update_struct(input: syn::ItemStruct) -> TokenStream {
    let idents: Vec<_> = input
        .fields
        .iter()
        .filter_map(|field| field.ident.to_owned())
        .collect();

    let set_quote = set_in_struct(idents.clone());
    let remove_quote = remove_in_struct(idents);
    let name = &input.ident;

    let result = quote! {
        impl UpdateTrait for #name {
            #set_quote
            #remove_quote
        }
    };

    result.into()
}

#[proc_macro_derive(Update)]
pub fn update_derive(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as Item);

    match input {
        Item::Struct(x) => update_struct(x),
        Item::Enum(x) => update_enum(x),
    }
}
