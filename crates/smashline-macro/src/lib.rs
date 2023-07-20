use proc_macro::TokenStream as TS;
use proc_macro2::{Group, Punct, Spacing, TokenStream, TokenTree};
use proc_macro_error::{abort, proc_macro_error};
use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
};

macro_rules! match_kw {
    ($input:ident;$($kw:path $([$token:tt])? => {$($t:tt)*}),*) => {
        $(
            if $input.peek($kw) {
                let _: $kw = $input.parse()?;
                let _: syn::Token![=] = $input.parse()?;
                $($t)*
            } else
        )*
        {
            Err(syn::Error::new($input.span(), concat!("Unexpected token, expected any of: [" $(, stringify!($kw))*, "]")))
        }
    }
}

mod kw {
    syn::custom_keyword!(agent);
    syn::custom_keyword!(script);
    syn::custom_keyword!(scripts);
    syn::custom_keyword!(category);
    syn::custom_keyword!(low_priority);
    syn::custom_keyword!(high_priority);

    syn::custom_keyword!(pre);
    syn::custom_keyword!(main);
    syn::custom_keyword!(end);
    syn::custom_keyword!(init);
    syn::custom_keyword!(exec);
    syn::custom_keyword!(exec_stop);
    syn::custom_keyword!(exec_post);
    syn::custom_keyword!(exit);
    syn::custom_keyword!(map_correction);
    syn::custom_keyword!(fix_camera);
    syn::custom_keyword!(fix_pos_slow);
    syn::custom_keyword!(check_damage);
    syn::custom_keyword!(check_attack);
    syn::custom_keyword!(on_change_lr);
    syn::custom_keyword!(leave_stop);
    syn::custom_keyword!(notify_event_gimmick);
}

enum AcmdAttribute {
    Agent(syn::Expr),
    Script(syn::Expr),
    Scripts(Vec<syn::Expr>),
    Category(syn::Expr),
    LowPriority,
    HighPriority,
}

impl Parse for AcmdAttribute {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        match_kw! {
            input;
            kw::agent [=] => { input.parse().map(Self::Agent) },
            kw::script [=] => { input.parse().map(Self::Script) },
            kw::scripts [=] => {
                let bracketed;
                syn::bracketed!(bracketed in input);
                Punctuated::<_, syn::token::Comma>::parse_terminated(&bracketed).map(|p| Self::Scripts(p.into_iter().collect()))
            },
            kw::category [=] => { input.parse().map(Self::Category) },
            kw::low_priority => { Ok(Self::LowPriority) },
            kw::high_priority => { Ok(Self::HighPriority) }
        }
    }
}

struct AcmdAttributes {
    agent: syn::Expr,
    scripts: Vec<syn::Expr>,
    category: syn::Expr,
    priority: syn::Expr,
}

fn name_to_cat(name: &str) -> Option<syn::Expr> {
    if name.starts_with("game_") {
        Some(syn::parse_quote!(::smashline::Acmd::Game))
    } else if name.starts_with("effect_") {
        Some(syn::parse_quote!(::smashline::Acmd::Effect))
    } else if name.starts_with("sound_") {
        Some(syn::parse_quote!(::smashline::Acmd::Sound))
    } else if name.starts_with("expression_") {
        Some(syn::parse_quote!(::smashline::Acmd::Expression))
    } else {
        None
    }
}

impl AcmdAttributes {
    fn parse_named(input: ParseStream) -> syn::Result<Self> {
        let attrs = Punctuated::<AcmdAttribute, syn::token::Comma>::parse_terminated(input)?;

        let mut agent = None;
        let mut scripts = vec![];
        let mut category = None;
        let mut priority = None;
        for attr in attrs {
            match attr {
                AcmdAttribute::Agent(expr) => agent = Some(expr),
                AcmdAttribute::Script(expr) => scripts.push(expr),
                AcmdAttribute::Scripts(exprs) => scripts.extend(exprs),
                AcmdAttribute::Category(expr) => category = Some(expr),
                AcmdAttribute::LowPriority => {
                    priority = Some(syn::parse_quote!(::smashline::Priority::Low))
                }
                AcmdAttribute::HighPriority => {
                    priority = Some(syn::parse_quote!(::smashline::Priority::High))
                }
            }
        }

        let Some(agent) = agent else {
            return Err(syn::Error::new(input.span(), "agent must be provided"));
        };

        if scripts.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "at least 1 script must be provided",
            ));
        }

        let priority =
            priority.unwrap_or_else(|| syn::parse_quote!(::smashline::Priority::Default));
        let category = if let Some(category) = category {
            category
        } else {
            for script in scripts.iter() {
                match script {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit_str),
                        ..
                    }) => {
                        if let Some(cat) = name_to_cat(lit_str.value().as_str()) {
                            category = Some(cat);
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(category) = category {
                category
            } else {
                return Err(syn::Error::new(
                    input.span(),
                    "could not infer acmd category from scripts, specify it manually",
                ));
            }
        };

        Ok(Self {
            agent,
            scripts,
            category,
            priority,
        })
    }

    fn parse_unnamed(input: ParseStream) -> syn::Result<Self> {
        let agent: syn::Expr = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let scripts: Vec<_> = if input.peek(syn::token::Bracket) {
            let bracketed;
            syn::bracketed!(bracketed in input);

            let scripts = Punctuated::<syn::Expr, syn::token::Comma>::parse_terminated(&bracketed)?;
            scripts.into_iter().collect()
        } else {
            let script: syn::Expr = input.parse()?;
            vec![script]
        };

        let mut category = None;
        for script in scripts.iter() {
            match script {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit_str),
                    ..
                }) => {
                    if let Some(cat) = name_to_cat(lit_str.value().as_str()) {
                        category = Some(cat);
                        break;
                    }
                }
                _ => {}
            }
        }

        let Some(category) = category else {
            return Err(syn::Error::new(
                input.span(),
                "could not infer acmd category from scripts, specify it manually",
            ));
        };

        Ok(Self {
            agent,
            scripts,
            category,
            priority: syn::parse_quote!(::smashline::Priority::Default),
        })
    }
}

impl Parse for AcmdAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Self::parse_named(input).or_else(|_| Self::parse_unnamed(input))
    }
}

fn returns_to_breaks(stream: TokenStream) -> TokenStream {
    let mut tokens = vec![];
    for token in stream {
        match token {
            TokenTree::Ident(ident) => {
                if ident.to_string() == "return" {
                    tokens.push(TokenTree::Ident(syn::Ident::new("break", ident.span())));
                    tokens.push(TokenTree::Punct(Punct::new('\'', Spacing::Joint)));
                    tokens.push(TokenTree::Ident(syn::Ident::new(
                        "__smashline_autogenerated_break",
                        ident.span(),
                    )));
                } else {
                    tokens.push(TokenTree::Ident(ident));
                }
            }
            TokenTree::Group(group) => {
                let delim = group.delimiter();
                let stream = returns_to_breaks(group.stream());
                tokens.push(TokenTree::Group(Group::new(delim, stream)));
            }
            other => tokens.push(other),
        }
    }

    TokenStream::from_iter(tokens)
}

fn map_function_block(func: &mut syn::ItemFn) -> syn::Result<()> {
    let block: syn::Block = syn::parse2(returns_to_breaks(func.block.to_token_stream()))?;
    let block = syn::parse_quote! {
        {
            let __smashline_autogenerated_ret = '__smashline_autogenerated_break: {
                #block
            };
            std::hint::black_box("__smashline_autogenerated_blackbox");
            __smashline_autogenerated_ret
        }
    };
    func.block = block;
    Ok(())
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn acmd_script(attr: TS, input: TS) -> TS {
    let attributes = syn::parse_macro_input!(attr as AcmdAttributes);

    let mut function = syn::parse_macro_input!(input as syn::ItemFn);

    function.sig.abi = Some(syn::Abi {
        extern_token: syn::token::Extern(function.sig.span()),
        name: Some(syn::LitStr::new("C", function.sig.span())),
    });
    function
        .sig
        .inputs
        .push(syn::parse_quote!(_variadic: &mut ::smashline::Variadic));

    if let Err(e) = map_function_block(&mut function) {
        abort!(e);
    }

    let vis = &function.vis;
    let ident = &function.sig.ident;

    let installs = attributes.scripts.iter().map(|script| {
        let agent = &attributes.agent;
        let category = &attributes.category;
        let priority = &attributes.priority;
        quote::quote! {
            ::smashline::api::install_acmd_script(
                ::smashline::AsHash40::as_hash40(#agent),
                ::smashline::AsHash40::as_hash40(#script),
                #category,
                #priority,
                #ident
            );
        }
    });

    let tokens = quote::quote! {
        #vis mod #ident {
            use super::*;
            pub fn install() {
                #(#installs)*
            }

            #[no_mangle]
            #function
        }
    };

    tokens.into()
}
