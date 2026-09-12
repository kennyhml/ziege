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
        media_types,
        collection,
        subobjects,
        capabilities,
        resources,
        container,
    } = arguments;
    let conditional_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        .collect::<Vec<_>>();

    let create_impl = capabilities.create.as_ref().map(|create| {
        let properties = &create.properties;
        quote! {
            #(#conditional_attrs)*
            impl crate::objects::ToXml for #properties {
                const XML_NAMESPACES: &'static [(&'static str, &'static str)] =
                    <#model as crate::objects::ToXml>::XML_NAMESPACES;
            }

            #(#conditional_attrs)*
            impl crate::objects::Create for #object {
                type Payload = #properties;

                #[doc(hidden)]
                fn prepare_payload<T>(
                    payload: &mut Self::Payload,
                    reference: &crate::objects::ObjectRef<T>,
                ) {
                    payload.assign_reference(reference);
                }
            }
        }
    });
    let source_impl = capabilities.source.is_some().then(|| {
        quote! {
            #(#conditional_attrs)*
            impl crate::objects::Source for #object {}
        }
    });
    let source_components_impl = capabilities.source_components.is_some().then(|| {
        quote! {
            #(#conditional_attrs)*
            impl crate::objects::SourceComponents for #object {}
        }
    });
    let extraction = if let Some(extractor) = resources {
        quote! { (#extractor)(properties) }
    } else if let Some(uri) = capabilities
        .source
        .as_ref()
        .and_then(|source| source.uri.as_ref())
    {
        quote! {
            crate::ResourceView::new(&properties.links)
                .with_main((#uri).as_str())
        }
    } else {
        quote! {
            crate::ResourceView::new(&properties.links)
        }
    };
    let container_impl = container.map(|container| {
        quote! {
            fn container(&self) -> Option<&crate::objects::AdvertisedObjectReference> {
                let properties = self;
                #container
            }
        }
    });
    let runtime_create = if capabilities.create.is_some() {
        quote! {
            Some(crate::objects::descriptors::CreateCodec::for_type::<#object>())
        }
    } else {
        quote!(None)
    };
    let runtime_run = if capabilities.run.is_some() {
        quote! {
            Some(<#object as crate::objects::ImmediateRun>::RUN)
        }
    } else {
        quote!(None)
    };
    let runtime_source = capabilities.source.is_some();
    let runtime_source_component = capabilities.source_components.is_some();
    let runtime_object_structure = capabilities.structure.is_some();
    let structure_impl = capabilities.structure.is_some().then(|| {
        quote! {
            #(#conditional_attrs)*
            impl crate::objects::Structure for #object {}
        }
    });
    let addressing_impl = if let Some((scheme, term)) = &collection {
        let subobject_descriptors = subobjects.iter().map(|subobject| {
            let child = &subobject.object;
            quote! {
                <#object as crate::objects::SubObject<#child>>::DESCRIPTOR
            }
        });
        let subobject_impls = subobjects.iter().map(|subobject| {
            let child = &subobject.object;
            let relation = &subobject.relation;
            let parent_variable = &subobject.parent_variable;
            quote! {
                #(#conditional_attrs)*
                impl crate::objects::SubObject<#child> for #object {
                    const DESCRIPTOR: crate::objects::SubObjectDescriptor =
                        crate::objects::SubObjectDescriptor::new(
                            <#child as crate::objects::ObjectType>::WORKBENCH_TYPE,
                            #relation,
                            #parent_variable,
                        );
                }
            }
        });
        quote! {
            #(#conditional_attrs)*
            impl crate::objects::private::PrimaryMetadata for #object {
                const SUBOBJECTS: &'static [crate::objects::SubObjectDescriptor] = &[
                    #(#subobject_descriptors),*
                ];
            }

            #(#conditional_attrs)*
            impl crate::objects::PrimaryObjectType for #object {
                const CATEGORY: crate::CategoryId = crate::CategoryId {
                    scheme: #scheme,
                    term: #term,
                };
            }

            #(#subobject_impls)*
        }
    } else {
        quote! {}
    };
    let runtime_addressing = if collection.is_some() {
        quote! {
            crate::objects::descriptors::ObjectAddressing::Primary {
                category: <#object as crate::objects::PrimaryObjectType>::CATEGORY,
                subobjects: <#object as crate::objects::private::PrimaryMetadata>::SUBOBJECTS,
            }
        }
    } else {
        quote!(crate::objects::descriptors::ObjectAddressing::Child)
    };
    Ok(quote! {
        #(#attrs)*
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
        #vis struct #object;

        #(#conditional_attrs)*
        impl crate::objects::private::Sealed for #object {}

        #(#conditional_attrs)*
        impl crate::Resources for #model {
            fn resources(&self) -> crate::ResourceView<'_> {
                let properties = self;
                #extraction
            }
        }

        #(#conditional_attrs)*
        impl crate::objects::Identity for #model {
            fn object_name(&self) -> &str {
                &self.name
            }

            fn workbench_type(&self) -> &crate::objects::GlobalWorkbenchType {
                &self.workbench_type
            }

            #container_impl
        }

        #(#conditional_attrs)*
        impl crate::objects::ObjectType for #object {
            type Properties = #model;

            const WORKBENCH_TYPE: crate::objects::GlobalWorkbenchType =
                crate::objects::GlobalWorkbenchType::new(#workbench_type);
            const MEDIA_TYPES: crate::MediaTypes = #media_types;
        }

        #addressing_impl

        #create_impl
        #source_impl
        #source_components_impl
        #structure_impl

        #(#conditional_attrs)*
        impl #object {
            pub(crate) const DESCRIPTOR: &'static crate::objects::descriptors::ObjectTypeDescriptor =
                &crate::objects::descriptors::ObjectTypeDescriptor::new(
                    <Self as crate::objects::ObjectType>::WORKBENCH_TYPE,
                    #runtime_addressing,
                    crate::objects::descriptors::PropertiesCodec::for_type::<Self>(),
                    crate::objects::descriptors::RuntimeCapabilities::new(
                        #runtime_create,
                        #runtime_run,
                        #runtime_source,
                        #runtime_source_component,
                        #runtime_object_structure,
                    ),
                );
        }
    })
}

struct ObjectTypeArguments {
    properties: Type,
    workbench_type: LitStr,
    media_types: Expr,
    collection: Option<(LitStr, LitStr)>,
    subobjects: Vec<SubObjectArgument>,
    capabilities: Capabilities,
    resources: Option<Expr>,
    container: Option<Expr>,
}

struct SubObjectArgument {
    object: Type,
    relation: LitStr,
    parent_variable: LitStr,
}

impl Parse for ObjectTypeArguments {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut properties = None;
        let mut workbench_type = None;
        let mut media_types = None;
        let mut collection = None;
        let mut subobject = None;
        let mut subobjects = None;
        let mut capabilities = None;
        let mut resources = None;
        let mut container = None;

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
            } else if key == "media_types" {
                reject_duplicate(&media_types, &key, "media_types")?;
                input.parse::<Token![=]>()?;
                media_types = Some(input.parse::<Expr>()?);
            } else if key == "collection" {
                reject_duplicate(&collection, &key, "collection")?;
                collection = Some(parse_collection(input)?);
            } else if key == "subobject" {
                reject_duplicate(&subobject, &key, "subobject")?;
                subobject = Some(key.span());
            } else if key == "subobjects" {
                reject_duplicate(&subobjects, &key, "subobjects")?;
                subobjects = Some(parse_subobjects(input)?);
            } else if key == "capabilities" {
                reject_duplicate(&capabilities, &key, "capabilities")?;
                capabilities = Some(parse_capabilities(input)?);
            } else if key == "resources" {
                reject_duplicate(&resources, &key, "resources")?;
                input.parse::<Token![=]>()?;
                resources = Some(input.parse::<Expr>()?);
            } else if key == "container" {
                reject_duplicate(&container, &key, "container")?;
                input.parse::<Token![=]>()?;
                container = Some(input.parse::<Expr>()?);
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
        let media_types = media_types.ok_or_else(|| {
            Error::new(Span::call_site(), "missing required `media_types` argument")
        })?;
        let capabilities = capabilities.ok_or_else(|| {
            Error::new(
                Span::call_site(),
                "missing required `capabilities` argument",
            )
        })?;

        match (&collection, subobject) {
            (Some(_), Some(span)) => {
                return Err(Error::new(
                    span,
                    "an object cannot be both a primary object and a subobject",
                ));
            }
            (None, None) => {
                return Err(Error::new(
                    Span::call_site(),
                    "missing object addressing: expected `collection(...)` or `subobject`",
                ));
            }
            _ => {}
        }
        let subobjects = subobjects.unwrap_or_default();
        if collection.is_none() && !subobjects.is_empty() {
            return Err(Error::new(
                Span::call_site(),
                "only primary objects can declare `subobjects(...)`",
            ));
        }
        if let Some(source_components) = capabilities.source_components
            && capabilities.source.is_none()
        {
            return Err(Error::new(
                source_components,
                "`SourceComponents` requires the `Source` capability",
            ));
        }

        Ok(Self {
            properties,
            workbench_type,
            media_types,
            collection,
            subobjects,
            capabilities,
            resources,
            container,
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

fn parse_subobjects(input: ParseStream<'_>) -> Result<Vec<SubObjectArgument>> {
    let content;
    parenthesized!(content in input);
    let mut subobjects = Vec::new();

    while !content.is_empty() {
        let object = content.parse::<Type>()?;
        let arguments;
        parenthesized!(arguments in content);
        let mut relation = None;
        let mut parent_variable = None;

        while !arguments.is_empty() {
            let key = arguments.parse::<Ident>()?;
            arguments.parse::<Token![=]>()?;
            if key == "relation" {
                reject_collection_duplicate(&relation, &key, "relation")?;
                relation = Some(arguments.parse::<LitStr>()?);
            } else if key == "parent_variable" {
                reject_collection_duplicate(&parent_variable, &key, "parent_variable")?;
                parent_variable = Some(arguments.parse::<LitStr>()?);
            } else {
                return Err(Error::new(
                    key.span(),
                    format!("unknown `subobjects` argument `{key}`"),
                ));
            }

            if arguments.is_empty() {
                break;
            }
            arguments.parse::<Token![,]>()?;
        }

        let relation = relation.ok_or_else(|| {
            Error::new(
                object.span(),
                "missing required subobject argument `relation`",
            )
        })?;
        let parent_variable = parent_variable.ok_or_else(|| {
            Error::new(
                object.span(),
                "missing required subobject argument `parent_variable`",
            )
        })?;
        if relation.value().trim().is_empty() {
            return Err(Error::new(
                relation.span(),
                "subobject relation cannot be empty",
            ));
        }
        if parent_variable.value().trim().is_empty() {
            return Err(Error::new(
                parent_variable.span(),
                "subobject parent variable cannot be empty",
            ));
        }
        if subobjects.iter().any(|existing: &SubObjectArgument| {
            existing.object.to_token_stream().to_string() == object.to_token_stream().to_string()
        }) {
            return Err(Error::new(object.span(), "duplicate subobject type"));
        }
        subobjects.push(SubObjectArgument {
            object,
            relation,
            parent_variable,
        });

        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }

    Ok(subobjects)
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
    source: Option<SourceCapability>,
    source_components: Option<Span>,
    structure: Option<Span>,
    run: Option<Span>,
}

struct CreateCapability {
    properties: Type,
}

struct SourceCapability {
    uri: Option<Expr>,
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
            if !arguments.is_empty() {
                arguments.parse::<Token![,]>()?;
            }
            if !arguments.is_empty() {
                return Err(arguments.error("unexpected `Create` capability argument"));
            }
            capabilities.create = Some(CreateCapability { properties });
        } else if capability == "Source" {
            if capabilities.source.is_some() {
                return Err(duplicate_capability(&capability));
            }
            let uri = if content.peek(syn::token::Paren) {
                let arguments;
                parenthesized!(arguments in content);
                let uri = arguments.parse::<Expr>()?;
                if !arguments.is_empty() {
                    arguments.parse::<Token![,]>()?;
                }
                if !arguments.is_empty() {
                    return Err(arguments.error("unexpected `Source` capability argument"));
                }
                Some(uri)
            } else {
                None
            };
            capabilities.source = Some(SourceCapability { uri });
        } else if capability == "SourceComponents" {
            reject_capability_arguments(&content, &capability)?;
            set_capability(&mut capabilities.source_components, capability)?;
        } else if capability == "Structure" {
            reject_capability_arguments(&content, &capability)?;
            set_capability(&mut capabilities.structure, capability)?;
        } else if capability == "Run" {
            reject_capability_arguments(&content, &capability)?;
            set_capability(&mut capabilities.run, capability)?;
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
/// A marked `Option<T>` becomes required `T` unless `optional` or a default is
/// specified, allowing sparse read payloads to retain required creation fields.
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
    let mut workbench_type_identity = None;
    let mut parent_context = None;

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
            } else if workbench_type_identity
                .replace(field_name.clone())
                .is_some()
            {
                return Err(Error::new(
                    field_name.span(),
                    "duplicate `workbench_type` identity field",
                ));
            }
        }
        if options.parent.is_some() && parent_context.replace(field_name.clone()).is_some() {
            return Err(Error::new(
                field_name.span(),
                "duplicate `parent` creation field",
            ));
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
        if options.default.is_some() || options.optional.is_some() || options.parent.is_some() {
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
        } else if options.optional.is_none()
            && options.default.is_none()
            && options.parent.is_none()
            && options.identity.is_none()
            && is_container_type(&source_type, "Option")
        {
            // A field can be absent in loaded properties but required for creation.
            let Type::Path(path) = &source_type else {
                unreachable!()
            };
            let syn::PathArguments::AngleBracketed(arguments) =
                &path.path.segments.last().unwrap().arguments
            else {
                unreachable!()
            };
            let Some(syn::GenericArgument::Type(inner)) = arguments.args.first() else {
                return Err(Error::new_spanned(&source_type, "expected Option<T>"));
            };
            attrs = without_serde_option(attrs, "skip_serializing_if")?;
            attrs = without_serde_option(attrs, "default")?;
            quote!(#inner)
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
        } else if options.default.is_some()
            || options.optional.is_some()
            || options.parent.is_some()
        {
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
    let workbench_type_identity = workbench_type_identity.ok_or_else(|| {
        Error::new(
            generated_name.span(),
            "`CreateProperties` requires an `identity` field named `workbench_type`",
        )
    })?;
    let assign_parent = parent_context.map(|parent| {
        quote! {
            if let Some(parent) = reference.parent_reference() {
                self.#parent = parent;
            }
        }
    });

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

            pub(crate) fn assign_reference<T>(
                &mut self,
                reference: &crate::objects::ObjectRef<T>,
            ) {
                self.#name_identity = reference.name().to_owned();
                self.#workbench_type_identity = reference.workbench_type().clone();
                #assign_parent
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
    parent: Option<Span>,
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
            } else if meta.path.is_ident("parent") {
                set_marker(&mut options.parent, &meta, "parent")
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
    if let (Some(_), Some(span)) = (options.parent, options.identity) {
        return Err(Error::new(
            span,
            "`parent` and `identity` cannot be combined",
        ));
    }
    if let Some(span) = options.identity {
        if field != "name" && field != "workbench_type" {
            return Err(Error::new(
                span,
                "`identity` is only valid on fields named `name` or `workbench_type`",
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
    } else if options.default.is_some() || options.optional.is_some() || options.parent.is_some() {
        attributes.push(quote!(default));
    }

    if options.identity.is_some() || options.parent.is_some() {
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
            media_types = MediaTypes::new(&["application/test+xml"]),
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
        assert!(expanded.contains("impl crate :: objects :: ObjectType for Class"));
        let media_types = quote! {
            const MEDIA_TYPES: crate::MediaTypes = MediaTypes::new(&["application/test+xml"]);
        }
        .to_string();
        assert!(expanded.contains(&media_types));
        assert!(!expanded.contains("impl crate :: objects :: Links"));
        assert!(expanded.contains("impl crate :: objects :: private :: Sealed for Class { }"));
        assert!(expanded.contains("impl crate :: Resources for ClassProperties"));
        let signature = quote! {
            fn resources(&self) -> crate::ResourceView<'_>
        }
        .to_string();
        assert!(expanded.contains(&signature));
        assert!(expanded.contains("let properties = self ;"));
        assert!(expanded.contains("crate :: ResourceView :: new (& properties . links)"));
        assert!(!expanded.contains("base"));
        assert!(!expanded.contains("AdtUri"));
        assert!(!expanded.contains("SnapshotResources"));
        assert!(!expanded.contains("with_main"));
        assert!(!expanded.contains("let mut resources"));
        assert!(expanded.contains("impl crate :: objects :: Identity for ClassProperties"));
        let identity_accessor = quote! {
            fn workbench_type(&self) -> &crate::objects::GlobalWorkbenchType {
                &self.workbench_type
            }
        }
        .to_string();
        assert!(expanded.contains(&identity_accessor));
        assert!(!expanded.contains("fn object_type"));
        assert!(!expanded.contains("fn container"));
        assert!(!expanded.contains("AssignObjectIdentity"));
        assert!(!expanded.contains("assign_identity"));
        assert!(!expanded.contains("assign_reference"));
        assert!(!expanded.contains("& mut"));
        assert!(expanded.contains("impl crate :: objects :: PrimaryObjectType for Class"));
        assert!(expanded.contains("impl crate :: objects :: private :: PrimaryMetadata for Class"));
        assert!(expanded.contains("type Properties = ClassProperties"));
        assert!(!expanded.contains("ObjectState"));
        assert!(!expanded.contains("AdtObject"));
        assert!(expanded.contains("ObjectTypeDescriptor :: new"));
        assert!(expanded.contains("PropertiesCodec :: for_type :: < Self >"));
        assert!(
            expanded.contains("RuntimeCapabilities :: new (None , None , false , false , false")
        );
        assert!(!expanded.contains("RuntimeObjectType"));
        assert!(!expanded.contains("impl crate :: objects :: Create for Class"));
    }

    #[test]
    fn object_type_emits_configured_capability_branches() {
        let expanded = expand_object_type(
            object_arguments(quote! {
                Create(ClassCreateProperties),
                Source(properties.source_uri),
                SourceComponents,
                Structure,
                Run,
            }),
            quote!(
                pub struct Class;
            ),
        )
        .unwrap()
        .to_string();

        assert!(expanded.contains("impl crate :: objects :: Create for Class"));
        assert!(expanded.contains("type Payload = ClassCreateProperties"));
        let prepare_payload = quote! {
            #[doc(hidden)]
            fn prepare_payload<T>(
                payload: &mut Self::Payload,
                reference: &crate::objects::ObjectRef<T>,
            ) {
                payload.assign_reference(reference);
            }
        }
        .to_string();
        assert!(expanded.contains(&prepare_payload));
        assert!(expanded.contains("impl crate :: objects :: ToXml for ClassCreateProperties"));
        assert!(!expanded.contains("fn set_identity"));
        assert!(!expanded.contains("AssignObjectIdentity"));
        assert!(!expanded.contains("assign_identity"));
        assert!(!expanded.contains("& mut self"));
        assert!(!expanded.contains("self . name ="));
        assert!(!expanded.contains("self . workbench_type ="));
        assert!(expanded.contains("CreateCodec :: for_type :: < Class >"));
        assert!(
            expanded.contains("< ClassProperties as crate :: objects :: ToXml > :: XML_NAMESPACES")
        );
        assert!(expanded.contains("impl crate :: objects :: Source for Class { }"));
        assert!(expanded.contains("impl crate :: objects :: SourceComponents for Class { }"));
        assert!(expanded.contains("impl crate :: objects :: Structure for Class"));
        assert!(!expanded.contains("_adapter"));
        assert!(!expanded.contains("source_parts"));
        assert!(expanded.contains("true , true , true"));
        assert!(expanded.contains(
            "crate :: ResourceView :: new (& properties . links) . with_main ((properties . source_uri) . as_str ())"
        ));
        assert!(expanded.contains("crate :: objects :: ImmediateRun > :: RUN"));
    }

    #[test]
    fn object_type_custom_resources_override_default_extraction() {
        for extractor in [
            quote!(family::resources),
            quote!(|properties: &ClassProperties| -> crate::ResourceView<'_> {
                extract(properties)
            }),
        ] {
            for source in [quote!(Source), quote!(Source(properties.source_uri))] {
                let arguments = object_arguments(quote!(#source, SourceComponents));
                let expanded = expand_object_type(
                    quote!(#arguments, resources = #extractor),
                    quote!(
                        pub struct Class;
                    ),
                )
                .unwrap()
                .to_string();

                let call = quote!((#extractor)(properties)).to_string();
                assert!(expanded.contains(&call));
                assert!(expanded.contains("impl crate :: Resources for ClassProperties"));
                assert!(
                    expanded.contains("impl crate :: objects :: private :: Sealed for Class { }")
                );
                assert!(!expanded.contains("base"));
                assert!(!expanded.contains("AdtUri"));
                assert!(expanded.contains("impl crate :: objects :: Source for Class { }"));
                assert!(
                    expanded.contains("impl crate :: objects :: SourceComponents for Class { }")
                );
                assert!(!expanded.contains("ResourceView :: new"));
                assert!(!expanded.contains("SnapshotResources"));
                assert!(!expanded.contains("with_main"));
                assert!(!expanded.contains("impl crate :: objects :: Links"));
            }
        }

        let arguments = object_arguments(quote!());
        let error = syn::parse2::<ObjectTypeArguments>(quote! {
            #arguments, resources = first, resources = second
        })
        .err()
        .unwrap()
        .to_string();
        assert!(error.contains("duplicate `object_type` argument `resources`"));
    }

    #[test]
    fn object_type_exposes_the_configured_borrowed_container() {
        let arguments = object_arguments(quote!());
        for container in [
            quote!(Some(&properties.container)),
            quote!(properties.owner.as_ref()),
        ] {
            let expanded = expand_object_type(
                quote!(#arguments, container = #container),
                quote!(
                    pub struct Class;
                ),
            )
            .unwrap()
            .to_string();
            let accessor = quote! {
                fn container(&self) -> Option<&crate::objects::AdvertisedObjectReference> {
                    let properties = self;
                    #container
                }
            }
            .to_string();
            assert!(expanded.contains(&accessor));
        }
        let error = syn::parse2::<ObjectTypeArguments>(quote! {
            #arguments, container = None, container = None
        })
        .err()
        .unwrap()
        .to_string();
        assert!(error.contains("duplicate `object_type` argument `container`"));
    }

    #[test]
    fn object_type_requires_media_types_and_rejects_duplicates() {
        let missing = syn::parse2::<ObjectTypeArguments>(quote! {
            properties = ClassProperties,
            workbench_type = "CLAS/OC",
            collection(scheme = "category", term = "classes"),
            capabilities()
        })
        .err()
        .unwrap()
        .to_string();
        assert!(missing.contains("missing required `media_types` argument"));

        let arguments = object_arguments(quote!());
        let duplicate = syn::parse2::<ObjectTypeArguments>(quote! {
            #arguments, media_types = OTHER_MEDIA_TYPES
        })
        .err()
        .unwrap()
        .to_string();
        assert!(duplicate.contains("duplicate `object_type` argument `media_types`"));
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
    fn object_type_generates_static_and_runtime_subobject_relationships() {
        let expanded = expand_object_type(
            quote! {
                properties = FunctionGroupProperties,
                workbench_type = "FUGR/F",
                media_types = MediaTypes::new(&["application/test+xml"]),
                collection(scheme = "functions", term = "groups"),
                subobjects(
                    FunctionModule(
                        relation = "functionmodules",
                        parent_variable = "groupname",
                    ),
                    FunctionGroupInclude(
                        relation = "includes",
                        parent_variable = "groupname",
                    ),
                ),
                capabilities()
            },
            quote!(
                pub struct FunctionGroup;
            ),
        )
        .unwrap()
        .to_string();

        assert!(
            expanded.contains(
                "impl crate :: objects :: SubObject < FunctionModule > for FunctionGroup"
            )
        );
        assert!(expanded.contains(
            "impl crate :: objects :: SubObject < FunctionGroupInclude > for FunctionGroup"
        ));
        assert!(expanded.contains("SubObjectDescriptor :: new"));
        assert!(expanded.contains("\"functionmodules\""));
        assert!(expanded.contains("\"groupname\""));
    }

    #[test]
    fn object_type_generates_subobjects_without_a_primary_collection() {
        let expanded = expand_object_type(
            quote! {
                properties = FunctionModuleProperties,
                workbench_type = "FUGR/FF",
                media_types = MediaTypes::new(&["application/test+xml"]),
                subobject,
                capabilities(Create(FunctionModuleCreateProperties), Source(properties.source_uri))
            },
            quote!(
                pub struct FunctionModule;
            ),
        )
        .unwrap()
        .to_string();

        assert!(!expanded.contains("PrimaryObjectType for FunctionModule"));
        assert!(expanded.contains("ObjectAddressing :: Child"));
        assert!(expanded.contains("impl crate :: objects :: Create for FunctionModule"));
    }

    #[test]
    fn object_type_requires_exactly_one_addressing_kind() {
        let missing = syn::parse2::<ObjectTypeArguments>(quote! {
            properties = Properties,
            workbench_type = "TEST/X",
            media_types = MediaTypes::new(&["application/test+xml"]),
            capabilities()
        })
        .err()
        .unwrap()
        .to_string();
        assert!(missing.contains("expected `collection(...)` or `subobject`"));

        let ambiguous = syn::parse2::<ObjectTypeArguments>(quote! {
            properties = Properties,
            workbench_type = "TEST/X",
            media_types = MediaTypes::new(&["application/test+xml"]),
            collection(scheme = "test", term = "objects"),
            subobject,
            capabilities()
        })
        .err()
        .unwrap()
        .to_string();
        assert!(ambiguous.contains("cannot be both a primary object and a subobject"));

        let empty_relation = syn::parse2::<ObjectTypeArguments>(quote! {
            properties = Properties,
            workbench_type = "TEST/X",
            media_types = MediaTypes::new(&["application/test+xml"]),
            collection(scheme = "test", term = "objects"),
            subobjects(Child(relation = "", parent_variable = "parent")),
            capabilities()
        })
        .err()
        .unwrap()
        .to_string();
        assert!(empty_relation.contains("relation cannot be empty"));
    }

    #[test]
    fn create_properties_selects_and_shapes_fields() {
        let expanded = expand_derive(quote! {
            #[doc = "Properties."]
            #[create_properties(name = ClassCreateProperties, doc = "Creation properties.")]
            #[serde(rename = "class:abapClass", deny_unknown_fields)]
            pub struct ClassProperties {
                #[for_create(identity, default)]
                #[serde(rename = "@adtcore:name")]
                pub(crate) name: String,
                #[for_create(identity, default = <Class as ObjectType>::WORKBENCH_TYPE)]
                #[serde(rename = "@adtcore:type")]
                pub(crate) workbench_type: GlobalWorkbenchType,
                #[for_create]
                #[serde(rename = "@adtcore:description", skip_serializing_if = "Option::is_none")]
                pub description: Option<String>,
                #[for_create(optional, doc = "Creation language.")]
                pub language: Language,
                #[for_create(optional)]
                pub template: Option<Template>,
                #[for_create(each = "source", default = vec![])]
                pub sources: Vec<Source>,
                #[for_create(with = "wire")]
                pub encoded: String,
                #[for_create(parent)]
                pub container: Reference,
                pub ignored: bool,
            }
        })
        .unwrap();

        assert!(expanded.contains("pub struct ClassCreateProperties"));
        assert!(expanded.contains("pub description : String"));
        assert!(!expanded.contains("pub description : Option"));
        assert!(expanded.contains("deny_unknown_fields"));
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
        assert!(
            expanded.contains("self . workbench_type = reference . workbench_type () . clone ()")
        );
        assert!(expanded.contains("pub (crate) name : String"));
        assert!(expanded.contains("pub (crate) workbench_type : GlobalWorkbenchType"));
        assert!(expanded.contains("serde (rename = \"@adtcore:type\")"));
        assert!(!expanded.contains("object_type"));
        assert!(!expanded.contains("ignored"));
        assert!(expanded.contains("impl ClassCreateProperties"));
        assert!(expanded.contains("pub fn builder () -> ClassCreatePropertiesBuilder"));
        assert!(expanded.contains("builder (default , setter (skip))"));
        assert_eq!(expanded.matches("setter (skip)").count(), 3);
        assert!(!expanded.contains("impl crate :: objects :: Identity"));
        assert!(!expanded.contains("assign_identity"));
        assert!(expanded.contains("pub (crate) fn assign_reference < T >"));
        assert!(expanded.contains("reference : & crate :: objects :: ObjectRef < T >"));
        let assign_parent = quote! {
            if let Some(parent) = reference.parent_reference() {
                self.container = parent;
            }
        }
        .to_string();
        assert!(expanded.contains(&assign_parent));
        assert!(!expanded.contains("impl crate :: objects :: PropertyModel"));
    }

    #[test]
    fn create_properties_assigns_reference_without_parent_field() {
        let expanded = expand_derive(quote! {
            #[create_properties(name = ClassCreateProperties)]
            pub struct ClassProperties {
                #[for_create(identity, default)]
                pub name: String,
                #[for_create(identity, default)]
                pub workbench_type: GlobalWorkbenchType,
            }
        })
        .unwrap();

        let assign_reference = quote! {
            pub(crate) fn assign_reference<T>(
                &mut self,
                reference: &crate::objects::ObjectRef<T>,
            ) {
                self.name = reference.name().to_owned();
                self.workbench_type = reference.workbench_type().clone();
            }
        }
        .to_string();
        assert!(expanded.contains(&assign_reference));
        assert!(expanded.contains("pub fn builder () -> ClassCreatePropertiesBuilder"));
        assert!(!expanded.contains("parent_reference"));
        assert!(!expanded.contains("impl crate :: objects :: Identity"));
        assert!(!expanded.contains("assign_identity"));
        assert!(!expanded.contains("impl ClassProperties"));
    }

    #[test]
    fn create_properties_rejects_invalid_field_options() {
        let old_identity_name = expand_derive(quote! {
            #[create_properties(name = Create)]
            struct Properties {
                #[for_create(identity, default)]
                name: String,
                #[for_create(identity, default)]
                object_type: GlobalWorkbenchType,
            }
        })
        .unwrap_err()
        .to_string();
        assert!(
            old_identity_name
                .contains("`identity` is only valid on fields named `name` or `workbench_type`")
        );

        let missing_workbench_type = expand_derive(quote! {
            #[create_properties(name = Create)]
            struct Properties {
                #[for_create(identity, default)]
                name: String,
            }
        })
        .unwrap_err()
        .to_string();
        assert!(
            missing_workbench_type
                .contains("`CreateProperties` requires an `identity` field named `workbench_type`")
        );

        let optional_identity = expand_derive(quote! {
            #[create_properties(name = Create)]
            struct Properties {
                #[for_create(identity, default, optional)]
                name: String,
                #[for_create(identity, default)]
                workbench_type: Type,
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
                workbench_type: Type,
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
                workbench_type: Type,
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
