//! Internal object-family declarations for `zadt`.

/// Declares a statically typed loaded ADT object and its runtime descriptor.
macro_rules! object_type {
    (
        $(#[$attribute:meta])*
        $visibility:vis type $object:ident = $model:ty;

        workbench_type = $workbench_type:literal,
        collection(
            scheme = $scheme:literal,
            term = $term:literal $(,)?
        ),
        capabilities(
            $($capability:ident $(($($arguments:tt)*))?),* $(,)?
        ) $(,)?
    ) => {
        $(#[$attribute])*
        $visibility type $object = $crate::objects::AdtObject<$model>;

        impl $crate::objects::private::Sealed for $object {}

        impl $crate::objects::ObjectType for $object {
            type Properties = $model;

            const WORKBENCH_TYPE: $crate::objects::GlobalWorkbenchType =
                $crate::objects::GlobalWorkbenchType::new($workbench_type);
            const CATEGORY: $crate::vocabulary::CategoryId = $crate::vocabulary::CategoryId {
                scheme: $scheme,
                term: $term,
            };
        }

        $(
            $crate::objects::macros::object_type!(
                @capability_impl $object, $capability $(($($arguments)*))?
            );
        )*

        impl $object {
            pub(crate) const DESCRIPTOR: &'static dyn $crate::objects::RuntimeObjectTypeDescriptor =
                &$crate::objects::ObjectTypeDescriptor::<Self>::new();
        }

        impl $crate::objects::descriptors::RuntimeObjectType for $object {
            fn run() -> Option<$crate::objects::RunCapability> {
                $crate::objects::macros::object_type!(
                    @run $object;
                    $($capability $(($($arguments)*))?,)*
                )
            }

            fn source_uri(properties: &Self::Properties) -> Option<&str> {
                $crate::objects::macros::object_type!(
                    @source_uri $object, properties;
                    $($capability $(($($arguments)*))?,)*
                )
            }

            fn source_component_uri<'a>(
                properties: &'a Self::Properties,
                name: &str,
            ) -> Option<&'a str> {
                $crate::objects::macros::object_type!(
                    @source_component_uri $object, properties, name;
                    $($capability $(($($arguments)*))?,)*
                )
            }

            fn properties_to_xml(
                object: &$crate::objects::ObjectRef<()>,
                media_type: &str,
                properties: serde_json::Value,
            ) -> Result<String, $crate::error::ObjectError> {
                $crate::objects::macros::object_type!(
                    @properties_to_xml $object, object, media_type, properties;
                    $($capability $(($($arguments)*))?,)*
                )
            }
        }
    };

    (@capability_impl $object:ident, Source) => {};
    (@capability_impl $object:ident, SourceComponents) => {};
    (@capability_impl $object:ident, Run) => {};
    (@capability_impl $object:ident, UpdateProperties) => {
        impl $crate::objects::UpdateProperties for $object {}
    };
    (@capability_impl $object:ident, $capability:ident $(($($arguments:tt)*))?) => {
        compile_error!(concat!("unknown object capability `", stringify!($capability), "`"));
    };

    (@run $object:ident; Run, $($rest:tt)*) => {
        Some(<$object as $crate::objects::ImmediateRun>::RUN)
    };
    (@run $object:ident; $capability:ident $(($($arguments:tt)*))?, $($rest:tt)*) => {
        $crate::objects::macros::object_type!(@run $object; $($rest)*)
    };
    (@run $object:ident;) => {
        None
    };

    (@source_uri $object:ident, $properties:ident; Source, $($rest:tt)*) => {
        <$object as $crate::objects::Source>::source_uri($properties)
    };
    (@source_uri $object:ident, $properties:ident; $capability:ident $(($($arguments:tt)*))?, $($rest:tt)*) => {
        $crate::objects::macros::object_type!(@source_uri $object, $properties; $($rest)*)
    };
    (@source_uri $object:ident, $properties:ident;) => {
        {
            let _ = $properties;
            None
        }
    };

    (@source_component_uri $object:ident, $properties:ident, $name:ident;
        SourceComponents, $($rest:tt)*) => {
        <$object as $crate::objects::SourceComponents>::source_component_uri($properties, $name)
    };
    (@source_component_uri $object:ident, $properties:ident, $name:ident;
        $capability:ident $(($($arguments:tt)*))?, $($rest:tt)*) => {
        $crate::objects::macros::object_type!(
            @source_component_uri $object, $properties, $name; $($rest)*
        )
    };
    (@source_component_uri $object:ident, $properties:ident, $name:ident;) => {
        {
            let _ = ($properties, $name);
            None
        }
    };

    (@properties_to_xml $object:ident, $resource:ident, $media_type:ident, $properties:ident;
        UpdateProperties, $($rest:tt)*) => {
        {
            let _ = $media_type;
            <$object as $crate::objects::UpdateProperties>::properties_to_xml(
                $resource,
                $properties,
            )
        }
    };
    (@properties_to_xml $object:ident, $resource:ident, $media_type:ident, $properties:ident;
        $capability:ident $(($($arguments:tt)*))?, $($rest:tt)*) => {
        $crate::objects::macros::object_type!(
            @properties_to_xml $object, $resource, $media_type, $properties;
            $($rest)*
        )
    };
    (@properties_to_xml $object:ident, $resource:ident, $media_type:ident, $properties:ident;) => {
        {
            let _ = ($resource, $media_type, $properties);
            Err($crate::objects::descriptors::unsupported_update(
                <$object as $crate::objects::ObjectType>::WORKBENCH_TYPE,
            ))
        }
    };
}

pub(crate) use object_type;
