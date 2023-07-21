use proc_macro2::TokenStream;
use syn::{
    parse::Parse,
    punctuated::{Pair, Punctuated},
    spanned::Spanned,
};

pub struct InTargetAttributes {
    module_name: syn::LitStr,
    offset: syn::LitInt,
}

impl Parse for InTargetAttributes {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let module_name: syn::LitStr = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let offset: syn::LitInt = input.parse()?;

        Ok(Self {
            module_name,
            offset,
        })
    }
}

impl InTargetAttributes {
    pub fn expand_api_call(&self, crate_tokens: TokenStream) -> TokenStream {
        let Self {
            module_name,
            offset,
        } = self;

        quote::quote! {
            #crate_tokens::api::get_target_function(
                #module_name,
                #offset
            )
        }
    }
}

fn map_to_bare_fn_arg(arg: &syn::FnArg) -> syn::Result<syn::BareFnArg> {
    match arg {
        syn::FnArg::Typed(typed) => Ok(syn::BareFnArg {
            attrs: typed.attrs.clone(),
            name: None,
            ty: (*typed.ty).clone(),
        }),
        syn::FnArg::Receiver(receiver) => {
            Err(syn::Error::new(receiver.span(), "receivers not supported"))
        }
    }
}

pub fn map_to_bare_fn(sig: &syn::Signature) -> syn::Result<syn::TypeBareFn> {
    let abi = syn::Abi {
        extern_token: syn::token::Extern(sig.fn_token.span()),
        name: Some(syn::LitStr::new("C", sig.fn_token.span())),
    };

    let mut inputs = Punctuated::new();
    for pair in sig.inputs.pairs() {
        let value = match pair {
            Pair::Punctuated(value, _) => map_to_bare_fn_arg(value)?,
            Pair::End(value) => map_to_bare_fn_arg(value)?,
        };

        inputs.push(value);
    }

    Ok(syn::TypeBareFn {
        lifetimes: None,
        unsafety: None,
        abi: Some(abi),
        fn_token: sig.fn_token.clone(),
        paren_token: sig.paren_token.clone(),
        inputs,
        // TODO: Add support for variadics
        variadic: None,
        output: sig.output.clone(),
    })
}

pub fn extract_args(sig: &syn::Signature) -> syn::Result<Vec<syn::Ident>> {
    let mut args = vec![];

    for input in sig.inputs.iter() {
        match input {
            syn::FnArg::Typed(syn::PatType { pat, .. }) => {
                let syn::Pat::Ident(syn::PatIdent { ident, .. }) = &**pat else {
                    return Err(syn::Error::new(pat.span(), "expected identifier pattern"));
                };
                args.push(ident.clone());
            }
            other => {
                return Err(syn::Error::new(other.span(), "expected identifier pattern"));
            }
        }
    }

    Ok(args)
}
