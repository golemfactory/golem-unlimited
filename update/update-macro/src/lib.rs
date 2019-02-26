#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream;
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

fn set_for_enum(idents: Vec<syn::Variant>, name: &Ident) -> impl quote::ToTokens {
    let sets = idents.into_iter().map(|item_enum| {
        let ident = item_enum.ident;
        let variants = item_enum.fields.into_iter().map(|x| x.ident.to_owned());
        //println!("{:?}", variants);

        quote! { #name::#ident(x) => {
            if stringify!(#ident) != key.next().ok_or("Clear failed - not declared for enum")? {
                return Err("Set failed - no such option in enum")
            }

            x.set(key, value)
        }}
    });

    quote! {
        fn set<I: Iterator<Item=String>>(&mut self, mut key: I, value: String) -> Result<(), &'static str> {
            match self {
                #(#sets,)*
                _ => Err("Update failed - unknown enum field"),
            }
        }
    }
}

fn remove_for_enum(idents: Vec<Ident>, name: &Ident) -> impl quote::ToTokens {
    let removes = idents.iter().map(|ident| {
        quote! { #name::#ident(x) => {
            if stringify!(#ident) != key.next().ok_or("Clear failed - not declared for enum")? {
                return Err("Set failed - no such option in enum")
            }

            x.remove(key)
        }}
    });

    quote! {
        fn remove<I: Iterator<Item=String>>(&mut self, mut key: I) -> Result<(), &'static str> {
            match self {
                #(#removes,)*
                _ => Err("Close failed - unknown enum field"),
            }
        }
    }
}

fn macro_for_enum(input: syn::ItemEnum) -> TokenStream {
    let idents: Vec<_> = input
        .variants
        .iter()
        .map(|field| field.ident.to_owned())
        .collect();

    let fields: Vec<_> = input.variants
        .iter()
        .map(|field| field.to_owned())
        .collect();

    let name = &input.ident;
    let set_quote = set_for_enum(fields, name);
    let remove_quote = remove_for_enum(idents, name);

    let result = quote! {
        impl UpdateTrait for #name {
            #set_quote
            #remove_quote
        }
    };

    result.into()
}

fn set_for_struct(idents: Vec<Ident>) -> impl quote::ToTokens {
    let sets = idents
        .into_iter()
        .map(|ident| quote! { stringify!(#ident) => self.#ident.set(key, value) });

    quote! {
        fn set<I: Iterator<Item=String>>(&mut self, mut key: I, value: String) -> Result<(), &'static str> {
            match key.next().ok_or("Update failed - not declared for struct")?.as_str() {
                #(#sets,)*
                _ => Err("Update failed - unknown struct field"),
            }
        }
    }
}

fn remove_for_struct(idents: Vec<Ident>) -> impl quote::ToTokens {
    let removes = idents
        .iter()
        .map(|ident| quote! { stringify!(#ident) => self.#ident.remove(key) });

    quote! {
        fn remove<I: Iterator<Item=String>>(&mut self, mut key: I) -> Result<(), &'static str> {
            match key.next().ok_or("Clear failed - not declared for struct")?.as_str() {
                #(#removes,)*
                _ => Err("Close failed - unknown struct field"),
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

    let set_quote = set_for_struct(idents.clone());
    let remove_quote = remove_for_struct(idents);
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
        Item::Struct(x) => macro_for_struct(x),
        Item::Enum(x) => macro_for_enum(x),
    }
}
