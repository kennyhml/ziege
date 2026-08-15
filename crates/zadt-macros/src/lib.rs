//! Internal object-family declarations for `zadt`.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Error, Fields, Ident, Item, LitStr, Result, Token, Type, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
};

#[proc_macro_attribute]
/// Declares a statically typed ADT object family and its runtime descriptor.
pub fn object_type(attributes: TokenStream, item: TokenStream) -> TokenStream {
    let attributes = parse_macro_input!(attributes as ObjectTypeAttributes);
    let item = parse_macro_input!(item as Item);

    expand_object_type(attributes, item)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Derives the names and paths for one closed set of secondary source components.
#[proc_macro_derive(SourceComponent, attributes(source_component))]
pub fn source_component(item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as DeriveInput);
    expand_source_component(item)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

struct ObjectTypeAttributes {
    workbench_type: LitStr,
    collection: Collection,
    has_source: bool,
    source_components: Option<Type>,
    run: bool,
    read_properties: Properties,
    update_properties: bool,
}

struct Collection {
    scheme: LitStr,
    term: LitStr,
}

struct Properties {
    model: Type,
}

impl Parse for ObjectTypeAttributes {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut workbench_type = None;
        let mut collection = None;
        let mut has_source = false;
        let mut source_components = None;
        let mut run = false;
        let mut read_properties = None;
        let mut update_properties = false;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "workbench_type" => {
                    input.parse::<Token![=]>()?;
                    set_once(&mut workbench_type, input.parse()?, &key)?;
                }
                "collection" => {
                    let content;
                    parenthesized!(content in input);
                    set_once(&mut collection, parse_collection(&content)?, &key)?;
                }
                "capabilities" => {
                    let content;
                    parenthesized!(content in input);
                    parse_capabilities(
                        &content,
                        &mut has_source,
                        &mut source_components,
                        &mut run,
                        &mut read_properties,
                        &mut update_properties,
                    )?;
                }
                _ => return Err(Error::new(key.span(), "unknown object type attribute")),
            }
            parse_optional_comma(input)?;
        }

        let workbench_type = required(workbench_type, input, "workbench_type")?;
        let collection = required(collection, input, "collection")?;
        let read_properties = required(read_properties, input, "ReadProperties capability")?;
        if source_components.is_some() && !has_source {
            return Err(Error::new(
                input.span(),
                "SourceComponents requires the HasSource capability",
            ));
        }

        Ok(Self {
            workbench_type,
            collection,
            has_source,
            source_components,
            run,
            read_properties,
            update_properties,
        })
    }
}

fn parse_collection(input: ParseStream<'_>) -> Result<Collection> {
    let mut scheme = None;
    let mut term = None;
    while !input.is_empty() {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        match key.to_string().as_str() {
            "scheme" => set_once(&mut scheme, input.parse()?, &key)?,
            "term" => set_once(&mut term, input.parse()?, &key)?,
            _ => return Err(Error::new(key.span(), "unknown collection attribute")),
        }
        parse_optional_comma(input)?;
    }
    Ok(Collection {
        scheme: required(scheme, input, "scheme")?,
        term: required(term, input, "term")?,
    })
}

fn parse_capabilities(
    input: ParseStream<'_>,
    has_source: &mut bool,
    source_components: &mut Option<Type>,
    run: &mut bool,
    read_properties: &mut Option<Properties>,
    update_properties: &mut bool,
) -> Result<()> {
    while !input.is_empty() {
        let capability: Ident = input.parse()?;
        match capability.to_string().as_str() {
            "HasSource" | "has_source" => {
                if *has_source {
                    return Err(Error::new(
                        capability.span(),
                        "duplicate HasSource capability",
                    ));
                }
                *has_source = true;
            }
            "SourceComponents" | "source_components" => {
                let content;
                parenthesized!(content in input);
                let component = content.parse()?;
                parse_optional_comma(&content)?;
                set_once(source_components, component, &capability)?;
            }
            "Run" | "run" => {
                if *run {
                    return Err(Error::new(capability.span(), "duplicate Run capability"));
                }
                *run = true;
            }
            "ReadProperties" | "read_properties" => {
                let content;
                parenthesized!(content in input);
                set_once(read_properties, parse_properties(&content)?, &capability)?;
            }
            "UpdateProperties" | "update_properties" => {
                if *update_properties {
                    return Err(Error::new(
                        capability.span(),
                        "duplicate UpdateProperties capability",
                    ));
                }
                *update_properties = true;
            }
            _ => return Err(Error::new(capability.span(), "unknown object capability")),
        }
        parse_optional_comma(input)?;
    }
    Ok(())
}

fn parse_properties(input: ParseStream<'_>) -> Result<Properties> {
    let mut model = None;
    while !input.is_empty() {
        let key: Ident = input.parse()?;
        match key.to_string().as_str() {
            "model" => {
                input.parse::<Token![=]>()?;
                set_once(&mut model, input.parse()?, &key)?;
            }
            _ => return Err(Error::new(key.span(), "unknown ReadProperties attribute")),
        }
        parse_optional_comma(input)?;
    }
    Ok(Properties {
        model: required(model, input, "model")?,
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T, key: &Ident) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(
            key.span(),
            format!("duplicate `{key}` attribute"),
        ));
    }
    Ok(())
}

fn required<T>(value: Option<T>, input: ParseStream<'_>, name: &str) -> Result<T> {
    value.ok_or_else(|| Error::new(input.span(), format!("missing `{name}` attribute")))
}

fn parse_optional_comma(input: ParseStream<'_>) -> Result<()> {
    if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
    } else if !input.is_empty() {
        return Err(input.error("expected `,`"));
    }
    Ok(())
}

fn expand_object_type(
    attributes: ObjectTypeAttributes,
    item: Item,
) -> Result<proc_macro2::TokenStream> {
    let ident = match &item {
        Item::Enum(item) => &item.ident,
        Item::Struct(item) => &item.ident,
        _ => {
            return Err(Error::new(
                item.span(),
                "object_type can only be applied to an enum or struct",
            ));
        }
    };

    let ObjectTypeAttributes {
        workbench_type,
        collection,
        has_source,
        source_components,
        run,
        read_properties,
        update_properties,
    } = attributes;
    let Collection { scheme, term } = collection;
    let Properties { model } = read_properties;

    let descriptor = format_ident!("__Zadt{}Descriptor", ident);
    let descriptor_static = format_ident!(
        "__ZADT_{}_DESCRIPTOR",
        ident.to_string().to_ascii_uppercase()
    );

    let source_impl = has_source.then(|| {
        quote! {
            impl crate::objects::HasSource for #ident {}
        }
    });
    let source_path = if has_source {
        quote!(Some(<#ident as crate::objects::HasSource>::SOURCE_PATH))
    } else {
        quote!(None)
    };
    let runtime_source_components = match source_components {
        Some(component) => quote!(#component::COMPONENT_PATHS),
        None => quote!(&[]),
    };
    let runtime_run = if run {
        quote!(Some(<#ident as crate::objects::ImmediateRun>::RUN))
    } else {
        quote!(None)
    };
    let update_properties_impl = update_properties.then(|| {
        quote! {
            impl crate::objects::UpdateProperties for #ident {}
        }
    });
    let runtime_to_xml = if update_properties {
        quote! {
            crate::objects::descriptors::properties_to_xml::<#ident>(
                object,
                media_type,
                payload,
            )
        }
    } else {
        quote! {
            Err(crate::objects::descriptors::unsupported_update(self.object_type()))
        }
    };
    Ok(quote! {
        #item

        impl crate::objects::private::Sealed for #ident {}

        impl crate::objects::ObjectType for #ident {
            const WORKBENCH_TYPE: crate::objects::GlobalWorkbenchType =
                crate::objects::GlobalWorkbenchType::new(#workbench_type);
            const CATEGORY: crate::vocabulary::CategoryId = crate::vocabulary::CategoryId {
                scheme: #scheme,
                term: #term,
            };

        }

        #source_impl

        impl crate::objects::ReadProperties for #ident {
            type Properties = #model;
        }

        #update_properties_impl

        #[doc(hidden)]
        #[derive(Debug)]
        struct #descriptor;

        #[doc(hidden)]
        static #descriptor_static: #descriptor = #descriptor;

        impl #ident {
            pub(crate) const DESCRIPTOR: &'static dyn crate::objects::RuntimeObjectTypeDescriptor =
                &#descriptor_static;
        }

        impl crate::objects::RuntimeObjectTypeDescriptor for #descriptor {
            fn object_type(&self) -> crate::objects::GlobalWorkbenchType {
                <#ident as crate::objects::ObjectType>::WORKBENCH_TYPE
            }

            fn category(&self) -> crate::vocabulary::CategoryId {
                <#ident as crate::objects::ObjectType>::CATEGORY
            }

            fn source_path(&self) -> Option<&'static [&'static str]> {
                #source_path
            }

            fn source_component_paths(
                &self,
            ) -> &'static [&'static [&'static str]] {
                #runtime_source_components
            }

            fn run(&self) -> Option<crate::objects::RunCapability> {
                #runtime_run
            }

            fn properties_request(
                &self,
                object: &crate::objects::ObjectRef<crate::objects::Erased>,
                version: Option<crate::objects::ObjectVersion>,
                client: &crate::client::Client<crate::client::Ready>,
            ) -> Result<crate::protocol::AdtRequest, crate::error::OperationError> {
                crate::objects::descriptors::properties_request::<#ident>(
                    object,
                    version,
                    client,
                )
            }

            fn properties_to_json(
                &self,
                object: &crate::objects::ObjectRef<crate::objects::Erased>,
                response: crate::operation::OperationResponse,
            ) -> Result<crate::JsonObjectProperties, crate::error::ResponseError> {
                crate::objects::descriptors::properties_to_json::<#ident>(object, response)
            }

            fn properties_to_xml(
                &self,
                object: &crate::objects::ObjectRef<crate::objects::Erased>,
                media_type: &'static str,
                payload: serde_json::Value,
            ) -> Result<String, crate::error::ObjectError> {
                #runtime_to_xml
            }
        }
    })
}

fn expand_source_component(item: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let ident = &item.ident;
    if !item.generics.params.is_empty() {
        return Err(Error::new(
            item.generics.span(),
            "SourceComponent cannot be derived for a generic enum",
        ));
    }

    let variants = match &item.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return Err(Error::new(
                item.span(),
                "SourceComponent can only be derived for an enum",
            ));
        }
    };
    if variants.is_empty() {
        return Err(Error::new(
            ident.span(),
            "SourceComponent requires at least one variant",
        ));
    }
    for variant in variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new(
                variant.fields.span(),
                "source component variants must not contain fields",
            ));
        }
    }

    let mut prefix = None;
    for attribute in &item.attrs {
        if !attribute.path().is_ident("source_component") {
            continue;
        }
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let value = meta.value()?.parse()?;
                if prefix.replace(value).is_some() {
                    return Err(meta.error("duplicate `prefix` attribute"));
                }
                Ok(())
            } else {
                Err(meta.error("unknown source component attribute"))
            }
        })?;
    }
    let prefix: LitStr = prefix.ok_or_else(|| {
        Error::new(
            ident.span(),
            "SourceComponent requires `#[source_component(prefix = \"...\")]`",
        )
    })?;

    let variant_idents = variants
        .iter()
        .map(|variant| &variant.ident)
        .collect::<Vec<_>>();
    let names = variant_idents
        .iter()
        .map(|variant| LitStr::new(&variant.to_string().to_ascii_lowercase(), variant.span()))
        .collect::<Vec<_>>();

    Ok(quote! {
        impl #ident {
            #[doc(hidden)]
            pub const COMPONENT_PATHS: &'static [&'static [&'static str]] = &[
                #(&[#prefix, #names]),*
            ];

            /// Returns the component name used by ADT.
            pub const fn as_str(self) -> &'static str {
                match self {
                    #(Self::#variant_idents => #names),*
                }
            }

            /// Parses a component name used by ADT.
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    #(#names => Some(Self::#variant_idents),)*
                    _ => None,
                }
            }

            /// Returns the component path relative to its owning object.
            pub const fn path(self) -> &'static [&'static str] {
                match self {
                    #(Self::#variant_idents => &[#prefix, #names]),*
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attributes(capabilities: &str) -> Result<ObjectTypeAttributes> {
        syn::parse_str(&format!(
            r#"
                workbench_type = "DTEL/DE",
                collection(scheme = "scheme", term = "term"),
                capabilities({capabilities})
            "#
        ))
    }

    #[test]
    fn parses_update_properties() {
        let attributes =
            attributes("ReadProperties(model = Properties), UpdateProperties").unwrap();

        assert!(attributes.update_properties);
    }

    #[test]
    fn rejects_duplicate_update_properties() {
        let error =
            attributes("ReadProperties(model = Properties), UpdateProperties, UpdateProperties")
                .err()
                .unwrap();

        assert!(
            error
                .to_string()
                .contains("duplicate UpdateProperties capability")
        );
    }
}
