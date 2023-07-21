use proc_macro2::TokenStream;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

mod kw {
    syn::custom_keyword!(agent);
    syn::custom_keyword!(script);
    syn::custom_keyword!(scripts);
    syn::custom_keyword!(category);
    syn::custom_keyword!(low_priority);
    syn::custom_keyword!(high_priority);
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
        crate::match_kw! {
            input;
            kw::agent [=] => input.parse().map(Self::Agent),
            kw::script [=] => input.parse().map(Self::Script),
            kw::scripts [=] => {
                let bracketed;
                syn::bracketed!(bracketed in input);
                Punctuated::<_, syn::token::Comma>::parse_terminated(&bracketed).map(|p| Self::Scripts(p.into_iter().collect()))
            },
            kw::category [=] => input.parse().map(Self::Category),
            kw::low_priority => Ok(Self::LowPriority),
            kw::high_priority => Ok(Self::HighPriority);
            _ => Err(syn::Error::new(input.span(), "unsupported item"))
        }
    }
}

pub struct AcmdAttributes {
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

fn literals_to_cat(exprs: &[syn::Expr]) -> Option<syn::Expr> {
    for expr in exprs.iter() {
        match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) => {
                if let Some(cat) = name_to_cat(lit_str.value().as_str()) {
                    return Some(cat);
                }
            }
            _ => {}
        }
    }

    None
}

impl AcmdAttributes {
    pub fn installer(&self, function: &syn::Ident, crate_tokens: TokenStream) -> TokenStream {
        let Self {
            agent,
            category,
            priority,
            scripts,
        } = self;

        quote::quote! {
            pub fn install() {
                #(
                    #crate_tokens::api::install_acmd_script(
                        #crate_tokens::AsHash40::as_hash40(#agent),
                        #crate_tokens::AsHash40::as_hash40(#scripts),
                        #category,
                        #priority,
                        #function
                    );
                )*
            }
        }
    }

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

        category = category.or_else(|| literals_to_cat(&scripts));

        let Some(category) = category else {
            return Err(syn::Error::new(
                input.span(),
                "could not infer acmd category from scripts"
            ));
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

        let Some(category) = literals_to_cat(&scripts) else {
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
