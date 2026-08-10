//! Internal object-family declarations for `zadt`.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Error, Fields, Ident, Item, LitInt, LitStr, Result, Token, Type,
    parenthesized,
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

/// Derives one closed set of secondary source components.
#[proc_macro_derive(SourceComponent, attributes(source_component))]
pub fn source_component(item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as DeriveInput);
    expand_source_component(item)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

struct ObjectTypeAttributes {
    workbench_type: LitStr,
    naming_policy: LitInt,
    collection: Collection,
    source: bool,
    source_components: Option<Type>,
    properties: Properties,
}

struct Collection {
    scheme: LitStr,
    term: LitStr,
}

struct Properties {
    media_version: Type,
    model: Type,
}

impl Parse for ObjectTypeAttributes {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut workbench_type = None;
        let mut naming_policy = None;
        let mut collection = None;
        let mut source = false;
        let mut source_components = None;
        let mut properties = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "workbench_type" => {
                    input.parse::<Token![=]>()?;
                    set_once(&mut workbench_type, input.parse()?, &key)?;
                }
                "naming_policy" => {
                    input.parse::<Token![=]>()?;
                    set_once(&mut naming_policy, input.parse()?, &key)?;
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
                        &mut source,
                        &mut source_components,
                        &mut properties,
                    )?;
                }
                _ => return Err(Error::new(key.span(), "unknown object type attribute")),
            }
            parse_optional_comma(input)?;
        }

        let workbench_type = required(workbench_type, input, "workbench_type")?;
        let naming_policy = required(naming_policy, input, "naming_policy")?;
        let collection = required(collection, input, "collection")?;
        let properties = required(properties, input, "Properties capability")?;
        if source_components.is_some() && !source {
            return Err(Error::new(
                input.span(),
                "SourceComponents requires the Source capability",
            ));
        }

        Ok(Self {
            workbench_type,
            naming_policy,
            collection,
            source,
            source_components,
            properties,
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
    source: &mut bool,
    source_components: &mut Option<Type>,
    properties: &mut Option<Properties>,
) -> Result<()> {
    while !input.is_empty() {
        let capability: Ident = input.parse()?;
        match capability.to_string().as_str() {
            "Source" | "source" => {
                if *source {
                    return Err(Error::new(capability.span(), "duplicate Source capability"));
                }
                *source = true;
            }
            "SourceComponents" | "source_components" => {
                let content;
                parenthesized!(content in input);
                let component = content.parse()?;
                parse_optional_comma(&content)?;
                set_once(source_components, component, &capability)?;
            }
            "Properties" | "properties" => {
                let content;
                parenthesized!(content in input);
                set_once(properties, parse_properties(&content)?, &capability)?;
            }
            _ => return Err(Error::new(capability.span(), "unknown object capability")),
        }
        parse_optional_comma(input)?;
    }
    Ok(())
}

fn parse_properties(input: ParseStream<'_>) -> Result<Properties> {
    let mut media_version = None;
    let mut model = None;
    while !input.is_empty() {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        match key.to_string().as_str() {
            "media_version" => set_once(&mut media_version, input.parse()?, &key)?,
            "model" => set_once(&mut model, input.parse()?, &key)?,
            _ => return Err(Error::new(key.span(), "unknown Properties attribute")),
        }
        parse_optional_comma(input)?;
    }
    Ok(Properties {
        media_version: required(media_version, input, "media_version")?,
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
        naming_policy,
        collection,
        source,
        source_components,
        properties,
    } = attributes;
    let Collection { scheme, term } = collection;
    let Properties {
        media_version,
        model,
    } = properties;

    let descriptor = format_ident!("__Zadt{}Descriptor", ident);
    let descriptor_static = format_ident!(
        "__ZADT_{}_DESCRIPTOR",
        ident.to_string().to_ascii_uppercase()
    );

    let source_impl = source.then(|| {
        quote! {
            impl crate::objects::Source for #ident {}
        }
    });
    let source_path = if source {
        quote!(Some(<#ident as crate::objects::Source>::SOURCE_PATH))
    } else {
        quote!(None)
    };
    let source_components_impl = source_components.as_ref().map(|component| {
        quote! {
            impl crate::objects::SourceComponents for #ident {
                type Component = #component;
            }
        }
    });
    let runtime_source_components = match source_components.as_ref() {
        Some(component) => {
            quote!(<#component as crate::objects::SourceComponentSet>::COMPONENTS)
        }
        None => quote!(&[]),
    };

    Ok(quote! {
        #item

        impl crate::objects::private::Sealed for #ident {}

        impl crate::objects::ObjectType for #ident {
            const WORKBENCH_TYPE: crate::objects::GlobalWorkbenchType =
                crate::objects::GlobalWorkbenchType::new(#workbench_type);
            const NAMING_POLICY: crate::objects::ObjectNamePolicy =
                crate::objects::ObjectNamePolicy::new(#naming_policy);
            const CATEGORY: crate::vocabulary::CategoryId = crate::vocabulary::CategoryId {
                scheme: #scheme,
                term: #term,
            };
        }

        #source_impl
        #source_components_impl

        impl crate::objects::ObjectProperties for #ident {
            type MediaVersion = #media_version;
            type Properties = #model;

            fn parse(
                resource: &crate::objects::ObjectRef<Self>,
                version: Self::MediaVersion,
                body: &[u8],
                etag: Option<crate::protocol::EntityTag>,
            ) -> Result<Self::Properties, crate::error::ResponseError> {
                <#model>::parse(resource, version, body, etag)
            }
        }

        #[doc(hidden)]
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

            fn naming_policy(&self) -> crate::objects::ObjectNamePolicy {
                <#ident as crate::objects::ObjectType>::NAMING_POLICY
            }

            fn source_path(&self) -> Option<&'static [&'static str]> {
                #source_path
            }

            fn source_components(
                &self,
            ) -> &'static [&'static dyn crate::objects::SourceComponent] {
                #runtime_source_components
            }

            fn resolve(
                &self,
                client: &crate::client::Client<crate::client::Ready>,
                name: &str,
            ) -> Result<crate::objects::ObjectRef, crate::error::ObjectError> {
                client.object::<#ident>(name).map(|reference| reference.erase())
            }

            fn normalize_reference(
                &self,
                reference: &crate::objects::ObjectRef,
            ) -> Result<crate::objects::ObjectRef, crate::error::ObjectError> {
                crate::objects::ObjectRef::<#ident>::from_parts(
                    reference.raw_name().to_owned(),
                    reference.uri().clone(),
                )
                .map(|reference| reference.erase())
            }

            fn properties(&self) -> &dyn crate::objects::RuntimeObjectProperties {
                self
            }
        }

        impl crate::objects::RuntimeObjectProperties for #descriptor {
            fn request(
                &self,
                resource: &crate::objects::ObjectRef,
                version: Option<crate::objects::ObjectVersion>,
                client: &crate::client::Client<crate::client::Ready>,
            ) -> Result<crate::protocol::AdtRequest, crate::error::OperationError> {
                let mut query =
                    crate::api::properties::ObjectPropertiesQuery::<#ident>::new(resource.retype());
                if let Some(version) = version {
                    query = query.version(version);
                }
                <crate::api::properties::ObjectPropertiesQuery<#ident> as
                    crate::operation::Operation<crate::client::Ready>>::request(&query, client)
            }

            fn decode(
                &self,
                resource: &crate::objects::ObjectRef,
                response: crate::operation::OperationResponse,
            ) -> Result<serde_json::Value, crate::error::ResponseError> {
                let query =
                    crate::api::properties::ObjectPropertiesQuery::<#ident>::new(resource.retype());
                let properties =
                    <crate::api::properties::ObjectPropertiesQuery<#ident> as
                        crate::operation::Operation<crate::client::Ready>>::decode(&query, response)?;
                serde_json::to_value(properties).map_err(Into::into)
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
        }

        impl crate::objects::SourceComponent for #ident {
            fn name(&self) -> &'static str {
                self.as_str()
            }

            fn path(&self) -> &'static [&'static str] {
                match self {
                    #(Self::#variant_idents => &[#prefix, #names]),*
                }
            }
        }

        impl crate::objects::SourceComponentSet for #ident {
            const COMPONENTS: &'static [&'static dyn crate::objects::SourceComponent] = &[
                #(&Self::#variant_idents),*
            ];
        }
    })
}
