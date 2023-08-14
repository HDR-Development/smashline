use inflector::Inflector;
use proc_macro::TokenStream as TS;
use proc_macro2::Span;
use proc_macro_crate::FoundCrate;
use proc_macro_error::{abort, emit_error, emit_warning, proc_macro_error};
use syn::{
    parse::Parse,
    punctuated::{Pair, Punctuated},
    spanned::Spanned,
};

mod kw {
    syn::custom_keyword!(struct_name);
    syn::custom_keyword!(hidden);
    syn::custom_keyword!(type_info);
}

enum ModuleAttributes {
    StructName(syn::Ident),
    HasTypeInfo,
}

struct MacroAttributes {
    struct_name: Option<syn::Ident>,
    has_type_info: bool,
}

impl Parse for MacroAttributes {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attributes = Punctuated::<ModuleAttributes, syn::Token![,]>::parse_terminated(input)?;

        let mut this = Self {
            struct_name: None,
            has_type_info: false,
        };

        for attribute in attributes {
            match attribute {
                ModuleAttributes::StructName(ident) => this.struct_name = Some(ident),
                ModuleAttributes::HasTypeInfo => this.has_type_info = true,
            }
        }

        Ok(this)
    }
}

impl Parse for ModuleAttributes {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.peek(kw::struct_name) {
            let _: kw::struct_name = input.parse()?;
            let _: syn::Token![=] = input.parse()?;
            let ident: syn::Ident = input.parse()?;

            Ok(Self::StructName(ident))
        } else if input.peek(kw::type_info) {
            let _: kw::type_info = input.parse()?;

            Ok(Self::HasTypeInfo)
        } else {
            Err(syn::Error::new(
                input.span(),
                "Unexpected input, expected 'struct_name'",
            ))
        }
    }
}

enum FunctionAttribute {
    Hidden,
}

impl Parse for FunctionAttribute {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.peek(kw::hidden) {
            let _: kw::hidden = input.parse()?;

            Ok(Self::Hidden)
        } else {
            Err(syn::Error::new(
                input.span(),
                "Unexpected input, expected 'hidden'",
            ))
        }
    }
}

struct FunctionAttributes {
    hidden: bool,
}

impl FunctionAttributes {
    fn join(self, other: Self) -> Self {
        Self {
            hidden: self.hidden | other.hidden,
        }
    }

    fn collect_over_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut this = Self { hidden: false };

        for attr in attrs {
            if !attr.path.is_ident("vtable") {
                continue;
            }

            this = this.join(attr.parse_args()?);
        }

        Ok(this)
    }
}

impl Parse for FunctionAttributes {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = Punctuated::<FunctionAttribute, syn::Token![,]>::parse_terminated(input)?;

        let mut this = Self { hidden: false };

        for attr in attrs.iter() {
            match attr {
                FunctionAttribute::Hidden => this.hidden = true,
            }
        }

        Ok(this)
    }
}

fn drain_filter<T>(container: &mut Vec<T>, condition: impl Fn(&T) -> bool) -> Vec<T> {
    let mut x = 0;
    let mut drained = vec![];
    while x < container.len() {
        if condition(&container[x]) {
            drained.push(container.remove(x));
        } else {
            x += 1;
        }
    }

    drained
}

fn ident_path(ident: syn::Ident) -> syn::Path {
    let mut path: Punctuated<syn::PathSegment, syn::token::Colon2> = Punctuated::new();
    let segment = syn::PathSegment {
        ident,
        arguments: syn::PathArguments::None,
    };
    path.push_value(segment);

    syn::Path {
        leading_colon: None,
        segments: path,
    }
}

fn make_ident_path(ident: &str, span: Span) -> syn::Path {
    ident_path(syn::Ident::new(ident, span))
}

fn map_to_bare_fn_arg(arg: &syn::FnArg, replace_self: Option<syn::Ident>) -> syn::BareFnArg {
    match arg {
        syn::FnArg::Typed(typed) => syn::BareFnArg {
            attrs: typed.attrs.clone(),
            name: None,
            ty: (*typed.ty).clone(),
        },
        syn::FnArg::Receiver(receiver) => {
            let path = replace_self.map_or_else(
                || make_ident_path("Self", receiver.self_token.span()),
                |ident| ident_path(ident),
            );

            let self_path = syn::TypePath { qself: None, path };

            let ty = if let Some((and, _)) = receiver.reference.as_ref() {
                let reference = syn::TypeReference {
                    and_token: and.clone(),
                    lifetime: None,
                    mutability: receiver.mutability.clone(),
                    elem: Box::new(syn::Type::Path(self_path)),
                };

                syn::Type::Reference(reference)
            } else {
                syn::Type::Path(self_path)
            };

            syn::BareFnArg {
                attrs: receiver.attrs.clone(),
                name: None,
                ty,
            }
        }
    }
}

fn map_to_bare_fn(
    foreign: &syn::ForeignItemFn,
    replace_self: Option<syn::Ident>,
) -> syn::TypeBareFn {
    // Ignore attrs + publicity
    let abi = syn::Abi {
        extern_token: syn::token::Extern(foreign.sig.fn_token.span()),
        name: Some(syn::LitStr::new("C", foreign.sig.fn_token.span())),
    };

    syn::TypeBareFn {
        lifetimes: None,
        unsafety: None,
        abi: Some(abi),
        fn_token: foreign.sig.fn_token.clone(),
        paren_token: foreign.sig.paren_token.clone(),
        inputs: foreign
            .sig
            .inputs
            .pairs()
            .map(|pair| match pair {
                Pair::Punctuated(value, comma) => Pair::Punctuated(
                    map_to_bare_fn_arg(value, replace_self.clone()),
                    comma.clone(),
                ),
                Pair::End(value) => Pair::End(map_to_bare_fn_arg(value, replace_self.clone())),
            })
            .collect(),
        // TODO: Add support for variadics
        variadic: None,
        output: foreign.sig.output.clone(),
    }
}

fn map_to_struct_field(foreign: &syn::ForeignItemFn) -> syn::Field {
    // warn on public
    let attrs = match FunctionAttributes::collect_over_attrs(&foreign.attrs) {
        Ok(attr) => attr,
        Err(e) => {
            emit_error!(e);
            FunctionAttributes { hidden: false }
        }
    };

    if attrs.hidden {
        match &foreign.vis {
            syn::Visibility::Inherited => {}
            other => {
                emit_warning!(
                    other.span(),
                    "explicit visibility specifiers are ignored for hidden vtable methods"
                );
            }
        }
    }

    let mut attrs = foreign.attrs.clone();
    let _ = drain_filter(&mut attrs, |attr| attr.path.is_ident("vtable"));

    syn::Field {
        attrs,
        vis: syn::Visibility::Inherited,
        ident: Some(foreign.sig.ident.clone()),
        colon_token: Some(syn::token::Colon(foreign.sig.ident.span())),
        ty: syn::Type::BareFn(map_to_bare_fn(foreign, None)),
    }
}

fn map_to_deref_method(foreign: &syn::ForeignItemFn) -> Option<syn::ItemFn> {
    let attrs = match FunctionAttributes::collect_over_attrs(&foreign.attrs) {
        Ok(attr) => attr,
        Err(e) => {
            emit_error!(e);
            return None;
        }
    };

    if attrs.hidden || foreign.sig.ident == "destructor" || foreign.sig.ident == "deleter" {
        return None;
    }

    let mut attrs = foreign.attrs.clone();
    let _ = drain_filter(&mut attrs, |attr| attr.path.is_ident("vtable"));

    let vis = &foreign.vis;
    let foreign_ident = &foreign.sig.ident;
    let inputs = &foreign.sig.inputs;
    let input_idents = inputs.iter().filter_map(|arg| match arg {
        syn::FnArg::Receiver(receiver) => Some(syn::Ident::new("self", receiver.self_token.span())),
        syn::FnArg::Typed(ty) => match &*ty.pat {
            syn::Pat::Ident(ident) => Some(ident.ident.clone()),
            other => {
                emit_error!(other.span(), "invalid argument name");
                None
            }
        },
    });
    let output = &foreign.sig.output;

    Some(syn::parse_quote! {
        #(#attrs)*
        #vis fn #foreign_ident(#inputs) #output {
            (self.#foreign_ident)(#(#input_idents),*)
        }
    })
}

fn map_to_getter(
    vtable_crate: syn::Ident,
    vtable_ident: syn::Ident,
    foreign: &syn::ForeignItemFn,
) -> syn::ItemFn {
    let get_ident = quote::format_ident!("get_{}", foreign.sig.ident);
    let vis = &foreign.vis;
    let foreign_ident = &foreign.sig.ident;
    let bare_fn = map_to_bare_fn(
        foreign,
        Some(syn::Ident::new("T", foreign.semi_token.span())),
    );
    syn::parse_quote! {
        #vis fn #get_ident<T>(&self) -> #bare_fn
        where
            T: #vtable_crate::VirtualClass,
            T: std::ops::DerefMut<Target = #vtable_ident>
        {
            #vtable_crate::vtable_read_guard::<_, T>(&self.0);
            unsafe { std::mem::transmute(self.0.#foreign_ident) }
        }
    }
}

fn map_to_setter(
    vtable_crate: syn::Ident,
    vtable_ident: syn::Ident,
    foreign: &syn::ForeignItemFn,
) -> syn::ItemFn {
    let set_ident = quote::format_ident!("set_{}", foreign.sig.ident);
    let vis = &foreign.vis;
    let foreign_ident = &foreign.sig.ident;
    let bare_fn = map_to_bare_fn(
        foreign,
        Some(syn::Ident::new("T", foreign.semi_token.span())),
    );
    syn::parse_quote! {
        #vis fn #set_ident<T>(&mut self, #foreign_ident: #bare_fn)
        where
            T: #vtable_crate::VirtualClass,
            T: std::ops::DerefMut<Target = #vtable_ident>
        {
            #vtable_crate::vtable_mutation_guard::<_, T>(&mut self.0);
            unsafe { self.0.#foreign_ident = std::mem::transmute(#foreign_ident) }
        }
    }
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn vtable(attr: TS, input: TS) -> TS {
    let attrs = syn::parse_macro_input!(attr as MacroAttributes);
    let input = syn::parse_macro_input!(input as syn::ItemMod);

    let Some((_, items)) = input.content else {
        abort!(
            input.span(),
            "'vtable' attribute must only be used on local modules"
        );
    };

    let mut vtable_procs = vec![];
    let mut helper_impls = vec![];

    for item in items {
        match item {
            syn::Item::Fn(item_fn) => {
                helper_impls.push(item_fn);
            }
            syn::Item::Verbatim(verbatim) => {
                let foreign_fn = match syn::parse2::<syn::ForeignItemFn>(verbatim) {
                    Ok(foreign) => foreign,
                    Err(e) => {
                        proc_macro_error::emit_error!(
                            e.span(),
                            "'vtable' decorated modules only supported function and foreign function items"
                        );
                        continue;
                    }
                };

                vtable_procs.push(foreign_fn);
            }
            other => {
                proc_macro_error::emit_error!(
                    other.span(),
                    "'vtable' decorated modules only supported function and foreign function items"
                );
            }
        }
    }

    let struct_name = attrs.struct_name.unwrap_or_else(|| {
        let name = input.ident.to_string();
        let pascal_case = name.to_pascal_case();
        syn::Ident::new(pascal_case.as_str(), input.ident.span())
    });

    let accessor_name = quote::format_ident!("{struct_name}VTableAccessor");
    let vtable_name = quote::format_ident!("{struct_name}VTable");

    let vtable_crate = match proc_macro_crate::crate_name("vtables") {
        Ok(FoundCrate::Itself) => syn::Ident::new("crate", Span::call_site()),
        Ok(FoundCrate::Name(name)) => syn::Ident::new(name.as_str(), Span::call_site()),
        Err(e) => abort!(syn::Error::new(Span::call_site(), format!("{e:?}"))),
    };

    let fields = vtable_procs.iter().map(|proc| map_to_struct_field(proc));

    let derefs = vtable_procs.iter().map(|proc| map_to_deref_method(proc));

    let getters = vtable_procs
        .iter()
        .map(|proc| map_to_getter(vtable_crate.clone(), vtable_name.clone(), proc));

    let setters = vtable_procs
        .iter()
        .map(|proc| map_to_setter(vtable_crate.clone(), vtable_name.clone(), proc));

    let accessor_trait = if attrs.has_type_info {
        quote::quote! {
            impl #vtable_crate::VTableAccessor for #accessor_name {
                const HAS_TYPE_INFO: bool = true;
            }
        }
    } else {
        quote::quote! {
            impl #vtable_crate::VTableAccessor for #accessor_name {
                const HAS_TYPE_INFO: bool = false;
            }
        }
    };

    quote::quote! {
        #[repr(C)]
        struct #vtable_name {
            #(#fields),*
        }

        #[allow(dead_code)]
        impl #vtable_name {
            #(#derefs)*

            #(#helper_impls)*
        }

        #[repr(transparent)]
        struct #accessor_name(&'static mut #vtable_name);

        #[allow(dead_code)]
        impl #accessor_name {
            pub fn inner(&self) -> &#vtable_name {
                self.0
            }

            pub fn inner_mut(&mut self) -> &mut #vtable_name {
                self.0
            }

            #(#getters)*

            #(#setters)*
        }

        #accessor_trait
    }
    .into()
}
