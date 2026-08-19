//! Internal object-model declarations for `zadt`.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, Error, Expr, Fields, GenericArgument, Ident, Item, ItemStruct,
    LitStr, Meta, PathArguments, Result, Token, Type, parenthesized,
};

#[proc_macro_attribute]
/// Declares an ADT object-family marker and its static and runtime capabilities.
pub fn object_type(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand_object_type(attribute.into(), item.into())
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand_object_type(attribute: TokenStream2, item: TokenStream2) -> Result<TokenStream2> {
    let arguments = syn::parse2::<ObjectTypeArguments>(attribute)?;
    let item = match syn::parse2::<Item>(item)? {
        Item::Struct(item) => item,
        item => {
            return Err(Error::new_spanned(
                item,
                "`object_type` can only be applied to a unit struct",
            ));
        }
    };

    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &item.generics,
            "`object_type` does not support generic marker structs",
        ));
    }
    if !matches!(item.fields, Fields::Unit) {
        return Err(Error::new_spanned(
            &item.fields,
            "`object_type` marker structs cannot have fields",
        ));
    }

    expand_object_type_item(item, arguments)
}

fn expand_object_type_item(
    item: ItemStruct,
    arguments: ObjectTypeArguments,
) -> Result<TokenStream2> {
    let ItemStruct {
        attrs,
        vis,
        ident: object,
        ..
    } = item;
    let ObjectTypeArguments {
        properties: model,
        workbench_type,
        scheme,
        term,
        capabilities,
    } = arguments;
    let conditional_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        .collect::<Vec<_>>();

    let create_impl = capabilities.create.as_ref().map(|create| {
        let properties = &create.properties;
        let version = &create.version;
        quote! {
            #(#conditional_attrs)*
            impl crate::objects::Create for #object {
                type CreateProperties = #properties;

                const CREATE_VERSION: <#properties as crate::objects::PropertyModel>::Version =
                    #version;
            }
        }
    });
    let update_impl = capabilities.update_properties.map(|_| {
        quote! {
            #(#conditional_attrs)*
            impl crate::objects::UpdateProperties for #object {}
        }
    });

    let create_media_type = if capabilities.create.is_some() {
        quote! {
            Some(<<#object as crate::objects::Create>::CreateProperties as crate::objects::PropertyModel>::media_type(
                <#object as crate::objects::Create>::CREATE_VERSION,
            ))
        }
    } else {
        quote!(None)
    };
    let creation_properties_to_xml = if capabilities.create.is_some() {
        quote! {
            <#object as crate::objects::Create>::creation_properties_to_xml(reference, properties)
        }
    } else {
        quote! {
            {
                let _ = (reference, properties);
                Err(crate::error::ObjectError::UnsupportedCapability {
                    object_type: <#object as crate::objects::ObjectType>::WORKBENCH_TYPE,
                    capability: "object creation",
                })
            }
        }
    };
    let run = if capabilities.run.is_some() {
        quote! {
            Some(<#object as crate::objects::ImmediateRun>::RUN)
        }
    } else {
        quote!(None)
    };
    let source_uri = if capabilities.source.is_some() {
        quote! {
            <#object as crate::objects::Source>::source_uri(properties)
        }
    } else {
        quote! {
            {
                let _ = properties;
                None
            }
        }
    };
    let source_component_uri = if capabilities.source_components.is_some() {
        quote! {
            <#object as crate::objects::SourceComponents>::source_component_uri(properties, name)
        }
    } else {
        quote! {
            {
                let _ = (properties, name);
                None
            }
        }
    };
    let has_object_structure = capabilities.structure.is_some();
    let properties_to_xml = if capabilities.update_properties.is_some() {
        quote! {
            {
                let _ = media_type;
                <#object as crate::objects::UpdateProperties>::properties_to_xml(
                    object,
                    properties,
                )
            }
        }
    } else {
        quote! {
            {
                let _ = (object, media_type, properties);
                Err(crate::objects::descriptors::unsupported_update(
                    <#object as crate::objects::ObjectType>::WORKBENCH_TYPE,
                ))
            }
        }
    };

    Ok(quote! {
        #(#attrs)*
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
        #vis struct #object;

        #(#conditional_attrs)*
        impl crate::objects::private::Sealed for #object {}

        #(#conditional_attrs)*
        impl crate::objects::ObjectState for #object {
            type Representation = #model;

            fn validate_representation(
                reference: &crate::objects::ObjectRef<Self>,
                media_type: &str,
                properties: &Self::Representation,
            ) -> Result<(), crate::error::ObjectError> {
                crate::objects::validate_typed_object::<Self>(reference, media_type, properties)
            }
        }

        #(#conditional_attrs)*
        impl crate::objects::ObjectType for #object {
            type Properties = #model;

            const WORKBENCH_TYPE: crate::objects::GlobalWorkbenchType =
                crate::objects::GlobalWorkbenchType::new(#workbench_type);
            const CATEGORY: crate::vocabulary::CategoryId = crate::vocabulary::CategoryId {
                scheme: #scheme,
                term: #term,
            };
        }

        #create_impl
        #update_impl

        #(#conditional_attrs)*
        impl #object {
            pub(crate) const DESCRIPTOR: &'static dyn crate::objects::RuntimeObjectTypeDescriptor =
                &crate::objects::ObjectTypeDescriptor::<Self>::new();
        }

        #(#conditional_attrs)*
        impl crate::objects::descriptors::RuntimeObjectType for #object {
            fn create_media_type() -> Option<&'static str> {
                #create_media_type
            }

            fn creation_properties_to_xml(
                reference: &crate::objects::ObjectRef<()>,
                properties: serde_json::Value,
            ) -> Result<Vec<u8>, crate::error::ObjectError> {
                #creation_properties_to_xml
            }

            fn run() -> Option<crate::objects::RunCapability> {
                #run
            }

            fn source_uri(properties: &Self::Properties) -> Option<&str> {
                #source_uri
            }

            fn source_component_uri<'a>(
                properties: &'a Self::Properties,
                name: &str,
            ) -> Option<&'a str> {
                #source_component_uri
            }

            fn has_object_structure() -> bool {
                #has_object_structure
            }

            fn properties_to_xml(
                object: &crate::objects::ObjectRef<()>,
                media_type: &str,
                properties: serde_json::Value,
            ) -> Result<Vec<u8>, crate::error::ObjectError> {
                #properties_to_xml
            }
        }
    })
}

struct ObjectTypeArguments {
    properties: Type,
    workbench_type: LitStr,
    scheme: LitStr,
    term: LitStr,
    capabilities: Capabilities,
}

impl Parse for ObjectTypeArguments {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut properties = None;
        let mut workbench_type = None;
        let mut collection = None;
        let mut capabilities = None;

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            if key == "properties" {
                reject_duplicate(&properties, &key, "properties")?;
                input.parse::<Token![=]>()?;
                properties = Some(input.parse::<Type>()?);
            } else if key == "workbench_type" {
                reject_duplicate(&workbench_type, &key, "workbench_type")?;
                input.parse::<Token![=]>()?;
                workbench_type = Some(input.parse::<LitStr>()?);
            } else if key == "collection" {
                reject_duplicate(&collection, &key, "collection")?;
                collection = Some(parse_collection(input)?);
            } else if key == "capabilities" {
                reject_duplicate(&capabilities, &key, "capabilities")?;
                capabilities = Some(parse_capabilities(input)?);
            } else {
                return Err(Error::new(
                    key.span(),
                    format!("unknown `object_type` argument `{key}`"),
                ));
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        let properties = properties.ok_or_else(|| {
            Error::new(Span::call_site(), "missing required `properties` argument")
        })?;
        let workbench_type = workbench_type.ok_or_else(|| {
            Error::new(
                Span::call_site(),
                "missing required `workbench_type` argument",
            )
        })?;
        let (scheme, term) = collection.ok_or_else(|| {
            Error::new(Span::call_site(), "missing required `collection` argument")
        })?;
        let capabilities = capabilities.ok_or_else(|| {
            Error::new(
                Span::call_site(),
                "missing required `capabilities` argument",
            )
        })?;

        if let (Some(source_components), None) =
            (capabilities.source_components, capabilities.source)
        {
            return Err(Error::new(
                source_components,
                "`SourceComponents` requires the `Source` capability",
            ));
        }

        Ok(Self {
            properties,
            workbench_type,
            scheme,
            term,
            capabilities,
        })
    }
}

fn reject_duplicate<T>(value: &Option<T>, key: &Ident, name: &str) -> Result<()> {
    if value.is_some() {
        Err(Error::new(
            key.span(),
            format!("duplicate `object_type` argument `{name}`"),
        ))
    } else {
        Ok(())
    }
}

fn parse_collection(input: ParseStream<'_>) -> Result<(LitStr, LitStr)> {
    let content;
    parenthesized!(content in input);
    let mut scheme = None;
    let mut term = None;

    while !content.is_empty() {
        let key = content.parse::<Ident>()?;
        if key == "scheme" {
            reject_collection_duplicate(&scheme, &key, "scheme")?;
            content.parse::<Token![=]>()?;
            scheme = Some(content.parse::<LitStr>()?);
        } else if key == "term" {
            reject_collection_duplicate(&term, &key, "term")?;
            content.parse::<Token![=]>()?;
            term = Some(content.parse::<LitStr>()?);
        } else {
            return Err(Error::new(
                key.span(),
                format!("unknown `collection` argument `{key}`"),
            ));
        }

        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }

    let scheme = scheme.ok_or_else(|| {
        Error::new(
            Span::call_site(),
            "missing required `collection` argument `scheme`",
        )
    })?;
    let term = term.ok_or_else(|| {
        Error::new(
            Span::call_site(),
            "missing required `collection` argument `term`",
        )
    })?;
    Ok((scheme, term))
}

fn reject_collection_duplicate<T>(value: &Option<T>, key: &Ident, name: &str) -> Result<()> {
    if value.is_some() {
        Err(Error::new(
            key.span(),
            format!("duplicate `collection` argument `{name}`"),
        ))
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct Capabilities {
    create: Option<CreateCapability>,
    source: Option<Span>,
    source_components: Option<Span>,
    structure: Option<Span>,
    run: Option<Span>,
    update_properties: Option<Span>,
}

struct CreateCapability {
    properties: Type,
    version: Expr,
}

fn parse_capabilities(input: ParseStream<'_>) -> Result<Capabilities> {
    let content;
    parenthesized!(content in input);
    let mut capabilities = Capabilities::default();

    while !content.is_empty() {
        let capability = content.parse::<Ident>()?;
        let span = capability.span();
        if capability == "Create" {
            if capabilities.create.is_some() {
                return Err(duplicate_capability(&capability));
            }
            let arguments;
            parenthesized!(arguments in content);
            let properties = arguments.parse::<Type>()?;
            arguments.parse::<Token![,]>()?;
            let version = arguments.parse::<Expr>()?;
            if !arguments.is_empty() {
                arguments.parse::<Token![,]>()?;
            }
            if !arguments.is_empty() {
                return Err(arguments.error("unexpected `Create` capability argument"));
            }
            capabilities.create = Some(CreateCapability {
                properties,
                version,
            });
        } else if capability == "Source" {
            reject_capability_arguments(&content, &capability)?;
            set_capability(&mut capabilities.source, capability)?;
        } else if capability == "SourceComponents" {
            reject_capability_arguments(&content, &capability)?;
            set_capability(&mut capabilities.source_components, capability)?;
        } else if capability == "Structure" {
            reject_capability_arguments(&content, &capability)?;
            set_capability(&mut capabilities.structure, capability)?;
        } else if capability == "Run" {
            reject_capability_arguments(&content, &capability)?;
            set_capability(&mut capabilities.run, capability)?;
        } else if capability == "UpdateProperties" {
            reject_capability_arguments(&content, &capability)?;
            set_capability(&mut capabilities.update_properties, capability)?;
        } else {
            return Err(Error::new(
                span,
                format!("unknown object capability `{capability}`"),
            ));
        }

        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }

    Ok(capabilities)
}

fn reject_capability_arguments(input: ParseStream<'_>, capability: &Ident) -> Result<()> {
    if input.peek(syn::token::Paren) {
        Err(Error::new(
            capability.span(),
            format!("capability `{capability}` does not accept arguments"),
        ))
    } else {
        Ok(())
    }
}

fn set_capability(slot: &mut Option<Span>, capability: Ident) -> Result<()> {
    if slot.is_some() {
        Err(duplicate_capability(&capability))
    } else {
        *slot = Some(capability.span());
        Ok(())
    }
}

fn duplicate_capability(capability: &Ident) -> Error {
    Error::new(
        capability.span(),
        format!("duplicate object capability `{capability}`"),
    )
}

#[proc_macro_derive(CreateProperties, attributes(create_properties, for_create))]
/// Generates a sparse creation model from marked fields in a complete properties model.
pub fn derive_create_properties(input: TokenStream) -> TokenStream {
    expand_create_properties(syn::parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand_create_properties(input: DeriveInput) -> Result<TokenStream2> {
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &input.generics,
            "`CreateProperties` does not support generic structs",
        ));
    }

    let CreatePropertiesArguments {
        name: generated_name,
        doc: generated_doc,
    } = parse_create_properties_arguments(&input.attrs)?;
    let mut container_attrs = copied_attrs(&input.attrs);
    if let Some(doc) = generated_doc {
        container_attrs.retain(|attr| !attr.path().is_ident("doc"));
        container_attrs.push(syn::parse_quote!(#[doc = #doc]));
    }
    let visibility = input.vis;
    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            fields => {
                return Err(Error::new_spanned(
                    fields,
                    "`CreateProperties` requires a struct with named fields",
                ));
            }
        },
        _ => {
            return Err(Error::new_spanned(
                &input.ident,
                "`CreateProperties` can only be derived for structs",
            ));
        }
    };

    let builder_name = format_ident!("{}Builder", generated_name);
    let mut generated_fields = Vec::new();
    let mut default_helpers = Vec::new();
    let mut name_identity = None;
    let mut object_type_identity = None;

    for (field_index, field) in fields.into_iter().enumerate() {
        let mut marker = None;
        for attr in &field.attrs {
            if attr.path().is_ident("for_create") {
                if marker.is_some() {
                    return Err(Error::new_spanned(attr, "duplicate `for_create` attribute"));
                }
                marker = Some(attr);
            }
        }
        let Some(marker) = marker else {
            continue;
        };

        let options = FieldOptions::parse(marker)?;
        let field_name = field.ident.expect("named fields have identifiers");
        validate_field_options(&field_name, &field.ty, &options)?;

        if options.identity.is_some() {
            if field_name == "name" {
                if name_identity.replace(field_name.clone()).is_some() {
                    return Err(Error::new(
                        field_name.span(),
                        "duplicate `name` identity field",
                    ));
                }
            } else if object_type_identity.replace(field_name.clone()).is_some() {
                return Err(Error::new(
                    field_name.span(),
                    "duplicate `object_type` identity field",
                ));
            }
        }

        if options.optional.is_some()
            && let Some(span) = find_serde_option(&field.attrs, "skip_serializing_if")?
        {
            return Err(Error::new(
                span,
                "an optional `for_create` field cannot copy an existing serde `skip_serializing_if` option",
            ));
        }

        let mut attrs = copied_attrs(&field.attrs);
        if options.default.is_some() || options.optional.is_some() {
            attrs = without_serde_option(attrs, "default")?;
        }
        if let Some(doc) = &options.doc {
            attrs.retain(|attr| !attr.path().is_ident("doc"));
            attrs.push(syn::parse_quote!(#[doc = #doc]));
        }
        let field_visibility = field.vis;
        let source_type = field.ty;
        let field_type = if options.optional.is_some() && !is_container_type(&source_type, "Option")
        {
            quote!(Option<#source_type>)
        } else {
            quote!(#source_type)
        };
        let builder_attr = builder_attribute(&options);
        let serde_default_attr = if let Some(expression) = &options.default_expression {
            let helper = format_ident!(
                "__zadt_{}_field_{}_default",
                generated_name.to_string().to_ascii_lowercase(),
                field_index
            );
            let helper_path = LitStr::new(&helper.to_string(), helper.span());
            default_helpers.push(quote! {
                fn #helper() -> #field_type {
                    #expression
                }
            });
            Some(quote! {
                #[serde(default = #helper_path)]
            })
        } else if options.default.is_some() || options.optional.is_some() {
            Some(quote! {
                #[serde(default)]
            })
        } else {
            None
        };
        let optional_serde_attr = options.optional.map(|_| {
            quote! {
                #[serde(skip_serializing_if = "Option::is_none")]
            }
        });
        let with_serde_attr = options.with.map(|path| {
            quote! {
                #[serde(with = #path)]
            }
        });

        generated_fields.push(quote! {
            #(#attrs)*
            #builder_attr
            #serde_default_attr
            #optional_serde_attr
            #with_serde_attr
            #field_visibility #field_name: #field_type,
        });
    }

    let name_identity = name_identity.ok_or_else(|| {
        Error::new(
            generated_name.span(),
            "`CreateProperties` requires an `identity` field named `name`",
        )
    })?;
    let object_type_identity = object_type_identity.ok_or_else(|| {
        Error::new(
            generated_name.span(),
            "`CreateProperties` requires an `identity` field named `object_type`",
        )
    })?;

    Ok(quote! {
        #(#default_helpers)*

        #[derive(
            ::derive_builder::Builder,
            Clone,
            Debug,
            ::serde::Deserialize,
            Eq,
            PartialEq,
            ::serde::Serialize,
        )]
        #[builder(pattern = "owned", setter(into))]
        #(#container_attrs)*
        #visibility struct #generated_name {
            #(#generated_fields)*
        }

        impl #generated_name {
            pub fn builder() -> #builder_name {
                #builder_name::default()
            }
        }

        impl crate::objects::CreationPropertyModel for #generated_name {
            fn set_identity<T>(&mut self, reference: &crate::objects::ObjectRef<T>) {
                self.#name_identity = reference.name().to_owned();
                self.#object_type_identity = reference.object_type().clone();
            }
        }
    })
}

struct CreatePropertiesArguments {
    name: Ident,
    doc: Option<LitStr>,
}

fn parse_create_properties_arguments(attrs: &[Attribute]) -> Result<CreatePropertiesArguments> {
    let mut helper_attr = None;
    for attr in attrs {
        if attr.path().is_ident("create_properties") {
            if helper_attr.is_some() {
                return Err(Error::new_spanned(
                    attr,
                    "duplicate `create_properties` attribute",
                ));
            }
            helper_attr = Some(attr);
        }
    }

    let attr = helper_attr.ok_or_else(|| {
        Error::new(
            Span::call_site(),
            "missing `#[create_properties(name = ...)]` attribute",
        )
    })?;
    let mut name = None;
    let mut doc = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("name") {
            if name.is_some() {
                return Err(meta.error("duplicate `create_properties` option `name`"));
            }
            name = Some(meta.value()?.parse::<Ident>()?);
            Ok(())
        } else if meta.path.is_ident("doc") {
            if doc.is_some() {
                return Err(meta.error("duplicate `create_properties` option `doc`"));
            }
            doc = Some(meta.value()?.parse::<LitStr>()?);
            Ok(())
        } else {
            Err(meta.error("unknown `create_properties` option"))
        }
    })?;
    Ok(CreatePropertiesArguments {
        name: name
            .ok_or_else(|| Error::new_spanned(attr, "missing `create_properties` option `name`"))?,
        doc,
    })
}

#[derive(Default)]
struct FieldOptions {
    optional: Option<Span>,
    identity: Option<Span>,
    default: Option<Span>,
    default_expression: Option<Expr>,
    each: Option<LitStr>,
    with: Option<LitStr>,
    doc: Option<LitStr>,
}

impl FieldOptions {
    fn parse(attr: &Attribute) -> Result<Self> {
        if matches!(&attr.meta, Meta::Path(_)) {
            return Ok(Self::default());
        }
        if !matches!(&attr.meta, Meta::List(_)) {
            return Err(Error::new_spanned(
                attr,
                "`for_create` options must be parenthesized",
            ));
        }

        let mut options = Self::default();
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("optional") {
                set_marker(&mut options.optional, &meta, "optional")
            } else if meta.path.is_ident("identity") {
                set_marker(&mut options.identity, &meta, "identity")
            } else if meta.path.is_ident("default") {
                if options.default.is_some() {
                    return Err(meta.error("duplicate `for_create` option `default`"));
                }
                options.default = Some(meta.path.span());
                if meta.input.peek(Token![=]) {
                    options.default_expression = Some(meta.value()?.parse::<Expr>()?);
                }
                Ok(())
            } else if meta.path.is_ident("each") {
                if options.each.is_some() {
                    return Err(meta.error("duplicate `for_create` option `each`"));
                }
                options.each = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else if meta.path.is_ident("with") {
                if options.with.is_some() {
                    return Err(meta.error("duplicate `for_create` option `with`"));
                }
                options.with = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else if meta.path.is_ident("doc") {
                if options.doc.is_some() {
                    return Err(meta.error("duplicate `for_create` option `doc`"));
                }
                options.doc = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else {
                Err(meta.error("unknown `for_create` option"))
            }
        })?;
        Ok(options)
    }
}

fn set_marker(
    slot: &mut Option<Span>,
    meta: &syn::meta::ParseNestedMeta<'_>,
    name: &str,
) -> Result<()> {
    if slot.is_some() {
        Err(meta.error(format!("duplicate `for_create` option `{name}`")))
    } else if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
        Err(meta.error(format!(
            "`for_create` option `{name}` does not take a value"
        )))
    } else {
        *slot = Some(meta.path.span());
        Ok(())
    }
}

fn validate_field_options(field: &Ident, ty: &Type, options: &FieldOptions) -> Result<()> {
    if let (Some(_), Some(span)) = (options.identity, options.optional) {
        return Err(Error::new(
            span,
            "`identity` and `optional` cannot be combined",
        ));
    }
    if let (Some(_), Some(each)) = (options.optional, &options.each) {
        return Err(Error::new(
            each.span(),
            "`optional` and `each` cannot be combined",
        ));
    }
    if let Some(span) = options.identity {
        if field != "name" && field != "object_type" {
            return Err(Error::new(
                span,
                "`identity` is only valid on fields named `name` or `object_type`",
            ));
        }
        if options.default.is_none() {
            return Err(Error::new(
                span,
                "an `identity` field requires `default` or `default = <expression>`",
            ));
        }
    }
    if let Some(each) = &options.each {
        if !is_container_type(ty, "Vec") {
            return Err(Error::new(
                each.span(),
                "`each` is only valid on fields with a syntactic `Vec<T>` type",
            ));
        }
        syn::parse_str::<Ident>(&each.value()).map_err(|_| {
            Error::new(
                each.span(),
                "the `each` value must be a valid Rust identifier",
            )
        })?;
    }
    Ok(())
}

fn is_container_type(ty: &Type, container: &str) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    if segment.ident != container {
        return false;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };
    arguments.args.len() == 1 && matches!(arguments.args.first(), Some(GenericArgument::Type(_)))
}

fn copied_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc") || attr.path().is_ident("serde"))
        .cloned()
        .collect()
}

fn without_serde_option(attrs: Vec<Attribute>, option: &str) -> Result<Vec<Attribute>> {
    attrs
        .into_iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("serde") {
                return Some(Ok(attr));
            }
            let options =
                match attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) {
                    Ok(options) => options,
                    Err(error) => return Some(Err(error)),
                };
            let options = options
                .into_iter()
                .filter(|meta| !meta.path().is_ident(option))
                .collect::<Punctuated<_, Token![,]>>();
            if options.is_empty() {
                None
            } else {
                Some(Ok(syn::parse_quote!(#[serde(#options)])))
            }
        })
        .collect()
}

fn find_serde_option(attrs: &[Attribute], option: &str) -> Result<Option<Span>> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let options = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        if let Some(meta) = options.iter().find(|meta| meta.path().is_ident(option)) {
            return Ok(Some(meta.path().span()));
        }
    }
    Ok(None)
}

fn builder_attribute(options: &FieldOptions) -> TokenStream2 {
    let mut attributes = Vec::new();
    if let Some(expression) = &options.default_expression {
        let expression = LitStr::new(&expression.to_token_stream().to_string(), expression.span());
        attributes.push(quote!(default = #expression));
    } else if options.default.is_some() || options.optional.is_some() {
        attributes.push(quote!(default));
    }

    if options.identity.is_some() {
        attributes.push(quote!(setter(skip)));
    } else if options.optional.is_some() {
        attributes.push(quote!(setter(strip_option)));
    }
    if let Some(each) = &options.each {
        attributes.push(quote!(setter(each(name = #each))));
    }

    if attributes.is_empty() {
        TokenStream2::new()
    } else {
        quote! {
            #[builder(#(#attributes),*)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object_arguments(capabilities: TokenStream2) -> TokenStream2 {
        quote! {
            properties = ClassProperties,
            workbench_type = "CLAS/OC",
            collection(scheme = "category", term = "classes"),
            capabilities(#capabilities)
        }
    }

    fn expand_derive(input: TokenStream2) -> Result<String> {
        expand_create_properties(syn::parse2(input)?).map(|tokens| tokens.to_string())
    }

    #[test]
    fn object_type_expands_empty_capabilities_directly() {
        let expanded = expand_object_type(
            object_arguments(quote!()),
            quote! {
                /// A class.
                pub struct Class;
            },
        )
        .unwrap()
        .to_string();

        assert!(expanded.contains("pub struct Class"));
        assert!(expanded.contains("impl crate :: objects :: ObjectState for Class"));
        assert!(expanded.contains("type Representation = ClassProperties"));
        assert!(expanded.contains("type Properties = ClassProperties"));
        assert!(!expanded.contains("AdtObject"));
        assert!(expanded.contains("capability : \"object creation\""));
        assert!(expanded.contains("crate :: objects :: descriptors :: unsupported_update"));
        assert!(!expanded.contains("impl crate :: objects :: Create for Class"));
    }

    #[test]
    fn object_type_emits_configured_capability_branches() {
        let expanded = expand_object_type(
            object_arguments(quote! {
                Create(ClassCreateProperties, ClassPropertiesVersion::V4),
                Source,
                SourceComponents,
                Run,
                UpdateProperties,
            }),
            quote!(
                pub struct Class;
            ),
        )
        .unwrap()
        .to_string();

        assert!(expanded.contains("impl crate :: objects :: Create for Class"));
        assert!(expanded.contains("impl crate :: objects :: UpdateProperties for Class"));
        assert!(
            expanded.contains("crate :: objects :: SourceComponents > :: source_component_uri")
        );
        assert!(expanded.contains("crate :: objects :: ImmediateRun > :: RUN"));
    }

    #[test]
    fn object_type_rejects_invalid_capabilities() {
        let missing_source = syn::parse2::<ObjectTypeArguments>(object_arguments(quote! {
            SourceComponents
        }))
        .err()
        .unwrap()
        .to_string();
        assert!(missing_source.contains("requires the `Source` capability"));

        let duplicate = syn::parse2::<ObjectTypeArguments>(object_arguments(quote! {
            Source, Source
        }))
        .err()
        .unwrap()
        .to_string();
        assert!(duplicate.contains("duplicate object capability `Source`"));

        let unknown = syn::parse2::<ObjectTypeArguments>(object_arguments(quote! {
            Delete
        }))
        .err()
        .unwrap()
        .to_string();
        assert!(unknown.contains("unknown object capability `Delete`"));
    }

    #[test]
    fn create_properties_selects_and_shapes_fields() {
        let expanded = expand_derive(quote! {
            #[doc = "Properties."]
            #[create_properties(name = ClassCreateProperties, doc = "Creation properties.")]
            #[serde(rename = "class:abapClass")]
            pub struct ClassProperties {
                #[for_create(identity, default)]
                #[serde(rename = "@adtcore:name")]
                pub name: String,
                #[for_create(identity, default = <Class as ObjectType>::WORKBENCH_TYPE)]
                pub object_type: GlobalWorkbenchType,
                #[for_create]
                pub description: String,
                #[for_create(optional, doc = "Creation language.")]
                pub language: Language,
                #[for_create(optional)]
                pub template: Option<Template>,
                #[for_create(each = "source", default = vec![])]
                pub sources: Vec<Source>,
                #[for_create(with = "wire")]
                pub encoded: String,
                pub ignored: bool,
            }
        })
        .unwrap();

        assert!(expanded.contains("pub struct ClassCreateProperties"));
        assert!(expanded.contains("doc = \"Creation properties.\""));
        assert!(expanded.contains("doc = \"Creation language.\""));
        assert!(expanded.contains("serde (default)"));
        assert!(expanded.contains("__zadt_classcreateproperties_field_1_default"));
        assert!(expanded.contains("pub language : Option < Language >"));
        assert!(expanded.contains("pub template : Option < Template >"));
        assert!(!expanded.contains("Option < Option < Template > >"));
        assert!(expanded.contains("skip_serializing_if = \"Option::is_none\""));
        assert!(expanded.contains("setter (each (name = \"source\"))"));
        assert!(expanded.contains("serde (with = \"wire\")"));
        assert!(expanded.contains("self . name = reference . name () . to_owned ()"));
        assert!(!expanded.contains("ignored"));
        assert!(!expanded.contains("impl crate :: objects :: PropertyModel"));
    }

    #[test]
    fn create_properties_rejects_invalid_field_options() {
        let optional_identity = expand_derive(quote! {
            #[create_properties(name = Create)]
            struct Properties {
                #[for_create(identity, default, optional)]
                name: String,
                #[for_create(identity, default)]
                object_type: Type,
            }
        })
        .unwrap_err()
        .to_string();
        assert!(optional_identity.contains("`identity` and `optional` cannot be combined"));

        let invalid_each = expand_derive(quote! {
            #[create_properties(name = Create)]
            struct Properties {
                #[for_create(identity, default)]
                name: String,
                #[for_create(identity, default)]
                object_type: Type,
                #[for_create(each = "value")]
                value: String,
            }
        })
        .unwrap_err()
        .to_string();
        assert!(invalid_each.contains("syntactic `Vec<T>`"));
    }

    #[test]
    fn create_properties_reports_serde_skip_conflicts() {
        let error = expand_derive(quote! {
            #[create_properties(name = Create)]
            struct Properties {
                #[for_create(identity, default)]
                name: String,
                #[for_create(identity, default)]
                object_type: Type,
                #[for_create(optional)]
                #[serde(skip_serializing_if = "custom")]
                value: Option<String>,
            }
        })
        .unwrap_err()
        .to_string();

        assert!(error.contains("existing serde `skip_serializing_if`"));
    }
}
