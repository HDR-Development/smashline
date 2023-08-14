use proc_macro2::TokenStream;
use syn::parse::Parse;


mod kw {
    syn::custom_keyword!(start);
    syn::custom_keyword!(end);
    syn::custom_keyword!(initialize);
    syn::custom_keyword!(finalize);
}

pub enum StateEvent {
    Initialize,
    Finalize,
    Start,
    End
}

impl Parse for StateEvent {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        crate::match_kw!(
            input;
            kw::initialize => Ok(Self::Initialize),
            kw::finalize => Ok(Self::Finalize),
            kw::start => Ok(Self::Start),
            kw::end => Ok(Self::End);
            _ => {
                Err(syn::Error::new(input.span(), "unexpected event kind"))
            }
        )
    }
}

pub struct StateCallbackAttributes {
    agent: Option<syn::Expr>,
    event: StateEvent
}

impl Parse for StateCallbackAttributes {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if let Ok(state) = input.parse() {
            return Ok(Self { agent: None, event: state });
        }

        let agent: syn::Expr = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let state = input.parse()?;

        Ok(Self {
            agent: Some(agent),
            event: state
        })
    }
}

impl StateCallbackAttributes {
    pub fn installer(&self, crate_tokens: TokenStream, name: &syn::Ident) -> TokenStream {
        let agent = if let Some(agent) = self.agent.as_ref() {
            quote::quote!(Some(#crate_tokens::AsHash40::as_hash40(#agent)))
        } else {
            quote::quote!(None)
        };

        let event_ident = match &self.event {
            StateEvent::Initialize => quote::quote!(Initialize),
            StateEvent::Finalize => quote::quote!(Finalize),
            StateEvent::Start => quote::quote!(Start),
            StateEvent::End => quote::quote!(End),
        };

        quote::quote! {
            #crate_tokens::api::install_state_callback(
                #agent,
                #crate_tokens::ObjectEvent::#event_ident,
                #name
            );
        }
    }
}