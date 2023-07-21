use proc_macro2::{Span, TokenStream};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
};

mod kw {
    syn::custom_keyword!(agent);
    syn::custom_keyword!(status);
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

struct ScriptLine {
    line_id: syn::Ident,
    arg_count: usize,
}

impl ScriptLine {
    fn from_name(name: &syn::Ident) -> syn::Result<Self> {
        let (line, count) = crate::match_suffix! {
            name.to_string();
            "pre" => ("Pre", 0),
            "main" => ("Main", 0),
            "end" => ("End", 0),
            "exec" => ("Exec", 0),
            "exec_stop" => ("ExecStop", 0),
            "post" => ("Post", 0),
            "exit" => ("Exit", 0),
            "map_correction" => ("MapCorrection", 0),
            "fix_camera" => ("FixCamera", 0),
            "fix_pos_slow" => ("FixPosSlow", 0),
            "check_damage" => ("CheckDamage", 1),
            "check_attack" => ("CheckAttack", 2),
            "on_change_lr" => ("OnChangeLr", 2),
            "leave_stop" => ("LeaveStop", 2),
            "notify_event_gimmick" => ("NotifyEventGimmick", 1);
            _ => {
                return Err(syn::Error::new(name.span(), "unable to determine line from name"));
            }
        };

        Ok(Self {
            line_id: syn::Ident::new(line, Span::call_site()),
            arg_count: count,
        })
    }
}

impl Parse for ScriptLine {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (line, count) = crate::match_kw! {
            input;
            kw::pre => ("Pre", 0),
            kw::main => ("Main", 0),
            kw::end => ("End", 0),
            kw::init => ("Init", 0),
            kw::exec => ("Exec", 0),
            kw::exec_stop => ("ExecStop", 0),
            kw::exec_post => ("Post", 0),
            kw::exit => ("Exit", 0),
            kw::map_correction => ("MapCorrection", 0),
            kw::fix_camera => ("FixCamera", 0),
            kw::fix_pos_slow => ("FixPosSlow", 0),
            kw::check_damage => ("CheckDamage", 1),
            kw::check_attack => ("CheckAttack", 2),
            kw::on_change_lr => ("OnChangeLr", 2),
            kw::leave_stop => ("LeaveStop", 2),
            kw::notify_event_gimmick => ("NotifyEventGimmick", 1);
            _ => {
                return Err(syn::Error::new(input.span(), "unsupported line id"));
            }
        };

        Ok(Self {
            line_id: syn::Ident::new(line, Span::call_site()),
            arg_count: count,
        })
    }
}

enum StatusAttribute {
    Agent(syn::Expr),
    Status(syn::Expr),
    Line(ScriptLine),
}

impl Parse for StatusAttribute {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        crate::match_kw! {
            input;
            kw::agent => input.parse().map(Self::Agent),
            kw::status => input.parse().map(Self::Status);
            _ => input.parse().map(Self::Line)

        }
    }
}

pub struct StatusAttributes {
    agent: syn::Expr,
    status: syn::Expr,
    line: Option<ScriptLine>,
}

impl StatusAttributes {
    pub fn try_set_line(&mut self, fn_name: &syn::Ident) -> syn::Result<()> {
        if self.line.is_some() {
            return Ok(());
        }

        self.line = Some(ScriptLine::from_name(fn_name)?);
        Ok(())
    }

    pub fn installer(
        &self,
        crate_tokens: TokenStream,
        name: &syn::Ident,
    ) -> syn::Result<TokenStream> {
        let Self {
            agent,
            status,
            line,
        } = self;

        let Some(line) = line.as_ref() else {
            return Err(syn::Error::new(name.span(), "no status line id found"));
        };

        let fn_name = match line.arg_count {
            0 => quote::quote!(install_basic_status_script),
            1 => quote::quote!(install_one_arg_status_script),
            2 => quote::quote!(install_two_arg_status_script),
            _ => {
                return Err(syn::Error::new(name.span(), "invalid status line id found"));
            }
        };

        let line_id = &line.line_id;

        Ok(quote::quote! {
            ::smashline::api::#fn_name(
                #crate_tokens::AsHash40::as_hash40(#agent),
                #crate_tokens::IntoLuaConst::into_lua_const(#status),
                #crate_tokens::StatusLine::#line_id as i32,
                #name
            );
        })
    }

    fn parse_named(input: ParseStream) -> syn::Result<Self> {
        let attrs = Punctuated::<StatusAttribute, syn::token::Comma>::parse_terminated(input)?;

        let mut agent = None;
        let mut status = None;
        let mut line = None;

        for attr in attrs {
            match attr {
                StatusAttribute::Agent(expr) => agent = Some(expr),
                StatusAttribute::Status(expr) => status = Some(expr),
                StatusAttribute::Line(line_) => line = Some(line_),
            }
        }

        let Some(agent) = agent else {
            return Err(syn::Error::new(input.span(), "agent must be provided"));
        };

        let Some(status) = status else {
            return Err(syn::Error::new(input.span(), "status must be provided"));
        };

        Ok(Self {
            agent,
            status,
            line,
        })
    }

    fn parse_unnamed(input: ParseStream) -> syn::Result<Self> {
        let agent: syn::Expr = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let status: syn::Expr = input.parse()?;

        let line = if input.peek(syn::token::Comma) {
            let _: syn::Token![,] = input.parse()?;
            let line: ScriptLine = input.parse()?;
            Some(line)
        } else {
            None
        };

        Ok(Self {
            agent,
            status,
            line,
        })
    }
}

impl Parse for StatusAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Self::parse_named(input).or_else(|_| Self::parse_unnamed(input))
    }
}

pub struct LineAttributes {
    agent: Option<syn::Expr>,
    line: ScriptLine,
}

impl LineAttributes {
    pub fn installer(
        &self,
        crate_tokens: TokenStream,
        name: &syn::Ident,
    ) -> syn::Result<TokenStream> {
        let Self { agent, line } = self;

        let fn_name = match line.arg_count {
            0 => quote::quote!(install_basic_line_callback),
            1 => quote::quote!(install_one_arg_line_callback),
            2 => quote::quote!(install_two_arg_line_callback),
            _ => {
                return Err(syn::Error::new(name.span(), "invalid line id found"));
            }
        };

        let line_id = &line.line_id;

        let agent = if let Some(agent) = agent.as_ref() {
            quote::quote!(Some(#crate_tokens::AsHash40::as_hash40(#agent)))
        } else {
            quote::quote!(None)
        };

        Ok(quote::quote! {
            ::smashline::api::#fn_name(
                #agent,
                #crate_tokens::StatusLine::#line_id as i32,
                #name
            );
        })
    }

    fn parse_named(input: ParseStream) -> syn::Result<Self> {
        let attrs = Punctuated::<StatusAttribute, syn::token::Comma>::parse_terminated(input)?;

        let mut agent = None;
        let mut line = None;

        for attr in attrs {
            match attr {
                StatusAttribute::Agent(expr) => agent = Some(expr),
                StatusAttribute::Status(expr) => {
                    return Err(syn::Error::new(
                        expr.span(),
                        "status field not supported for line callbacks",
                    ));
                }
                StatusAttribute::Line(line_) => line = Some(line_),
            }
        }

        let Some(line) = line else {
            return Err(syn::Error::new(input.span(), "line must be provided"));
        };

        Ok(Self { agent, line })
    }

    fn parse_unnamed(input: ParseStream) -> syn::Result<Self> {
        let mut line = None;
        let mut agent = None;

        if let Ok(line_) = input.parse() {
            line = Some(line_);
        } else if let Ok(agent_) = input.parse() {
            agent = Some(agent_);
        } else {
            return Err(syn::Error::new(input.span(), "unexpected input"));
        }

        if line.is_none() {
            let _: syn::Token![,] = input.parse()?;
            line = Some(input.parse()?);
        }

        let line = line.unwrap();

        Ok(Self { agent, line })
    }
}

impl Parse for LineAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Self::parse_named(input).or_else(|_| Self::parse_unnamed(input))
    }
}
