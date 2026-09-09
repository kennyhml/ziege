use zadt::ObjectName;

use crate::{ProjectionError, validate};

/// Describes where a name template section is supplied from.
///
///
#[derive(Clone, Copy, Debug)]
pub(crate) enum NameSource {
    Object,
    Parent,
}

impl NameSource {
    fn select_from<'a, 'name>(
        &self,
        object: &'a ObjectName<'name>,
        parent: Option<&'a ObjectName<'name>>,
    ) -> Result<&'a ObjectName<'name>, ProjectionError> {
        match self {
            NameSource::Object => Ok(object),
            NameSource::Parent => parent.ok_or_else(|| ProjectionError::InvalidAffField {
                field: "parent",
                message: "filename template requires a parent name".to_owned(),
            }),
        }
    }
}

/// Filename syntax and name bindings, independent of file content or snapshots.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FilenameTemplate {
    template: &'static str,
    names: &'static [(&'static str, NameSource)],
}

impl FilenameTemplate {
    pub const fn new(template: &'static str) -> Self {
        Self {
            template,
            names: &[("name", NameSource::Object)],
        }
    }

    pub const fn with_names(mut self, names: &'static [(&'static str, NameSource)]) -> Self {
        self.names = names;
        self
    }

    pub const fn template(&self) -> &'static str {
        self.template
    }

    pub fn requires_parent(&self) -> bool {
        self.names.iter().any(|(placeholder, source)| {
            matches!(source, NameSource::Parent)
                && self.template.contains(&format!("<{placeholder}>"))
        })
    }

    /// Substitutes the declared name bindings and optional language tag.
    ///
    /// By default `<name>` uses the current object. A child spec can instead bind
    /// `name` to its parent and another placeholder such as `fmname` to the child.
    ///
    /// `<lang>` remains a language tag, required only when present in the template.
    ///
    /// Name-bearing filename components are encoded as complete ABAP names. Literal
    /// prefixes and suffixes belong inside the namespace. For `/ACME/DEMO`,
    /// `sapl<name>` becomes `(acme)sapldemo` and `l<name>top` becomes `(acme)ldemotop`.
    pub fn substitute(
        &self,
        object: ObjectName<'_>,
        parent: Option<ObjectName<'_>>,
        language: Option<&str>,
    ) -> Result<String, ProjectionError> {
        // If we are given a language, we expect the object to have a
        // placeholder for it and vice versa.
        let template = if self.template.contains("<lang>") {
            let language = language.ok_or(ProjectionError::MissingLanguage {
                template: self.template,
            })?;
            crate::validate::validate_language(language)?;
            self.template.replace("<lang>", language)
        } else {
            if let Some(language) = language {
                return Err(ProjectionError::UnexpectedLanguage {
                    template: self.template,
                    language: language.to_owned(),
                });
            }
            self.template.to_owned()
        };

        // Go through each section and merge
        let mut filename = String::new();
        for (index, component) in template.split('.').enumerate() {
            if index != 0 {
                filename.push('.');
            }

            let mut component = component.to_owned();
            let mut namespace = None;
            let mut has_name = false;

            for (placeholder, source) in self.names {
                let placeholder = format!("<{placeholder}>");
                // Care that we need to check `contains`, not equal!
                // A component is not always solely an object name. e.g: `SAPL<name>`
                if !component.contains(&placeholder) {
                    continue;
                }

                let name = source.select_from(&object, parent.as_ref())?;
                if let Some(prefix) = name.namespace() {
                    namespace = Some(prefix);
                }
                component = component.replace(&placeholder, name.local_name());
                has_name = true;
            }

            if let Some(namespace) = namespace {
                filename.push('(');
                filename.push_str(&namespace.to_ascii_lowercase());
                filename.push(')');
            }

            if has_name {
                component.make_ascii_lowercase();
            }
            filename.push_str(&component);
        }
        Ok(filename)
    }
}

/// Encodes an ABAP object name for use in an AFF filename.
///
/// ASCII letters are lowercased. A namespace written as `/NAMESPACE/NAME`
/// becomes `(namespace)name`. The result is only the encoded object name,
/// without a format suffix such as `.clas.json`.
///
/// # Examples
///
/// ```
/// use zaff::encode_object_name;
///
/// assert_eq!(encode_object_name("Z_MY_CLASS")?, "z_my_class");
/// assert_eq!(encode_object_name("/ACME/MY_CLASS")?, "(acme)my_class");
/// # Ok::<(), zaff::ProjectionError>(())
/// ```
///
/// # Errors
///
/// Returns [`ProjectionError::InvalidObjectName`] for empty names, leading or
/// trailing whitespace, control characters, backslashes, angle brackets, or
/// the names `.` and `..`. Namespace components must be nonempty. Slashes are
/// only accepted as the two namespace separators, and parentheses are rejected.
/// Already encoded AFF names such as `(acme)my_class` are not valid inputs.
///
/// This is a filename conversion, not a complete check of SAP object naming
/// rules or a URI encoder. It preserves non-ASCII characters without case conversion.
pub fn encode_object_name(object_name: &str) -> Result<String, ProjectionError> {
    let name = validate::validate_object_name(object_name)?;
    if let Some(namespace) = name.namespace() {
        Ok(format!(
            "({}){}",
            namespace.to_ascii_lowercase(),
            name.local_name().to_ascii_lowercase()
        ))
    } else {
        Ok(name.local_name().to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn filename_placeholder_meanings_come_only_from_declared_bindings() {
        let child = ObjectName::parse("/CHILD/MODULE").unwrap();
        let parent = Some(ObjectName::parse("/OWNER/GROUP").unwrap());
        let object = ObjectName::parse("Z_OBJECT").unwrap();
        let template = FilenameTemplate::new("<container>.fugr.<member>.func.json").with_names(&[
            ("container", NameSource::Parent),
            ("member", NameSource::Object),
        ]);
        assert_eq!(
            template.substitute(child, parent, None).unwrap(),
            "(owner)group.fugr.(child)module.func.json"
        );

        let template = FilenameTemplate::new("<name>.abap")
            .with_names(&[("name", NameSource::Object), ("unused", NameSource::Parent)]);
        assert!(!template.requires_parent());
        assert_eq!(
            template.substitute(object, None, None).unwrap(),
            "z_object.abap"
        );
    }

    #[test]
    fn filename_templates_substitute_context_and_repeated_placeholders() {
        for (template, name, parent, language, expected) in [
            (
                "<name>.fugr.sapl<name>.reps.abap",
                "Z_EXAMPLE",
                None,
                None,
                "z_example.fugr.saplz_example.reps.abap",
            ),
            (
                "<name>.fugr.sapl<name>.sapl<name>.reps.json",
                "/ACME/DEMO",
                None,
                None,
                "(acme)demo.fugr.(acme)sapldemo.(acme)sapldemo.reps.json",
            ),
            (
                "<name>.fugr.<fmname>.func.json",
                "Z_MODULE",
                Some("Z_GROUP"),
                None,
                "z_group.fugr.z_module.func.json",
            ),
            (
                "<name>.<name>.<fmname>",
                "/ACME/MODULE",
                Some("/ACME/GROUP"),
                None,
                "(acme)group.(acme)group.(acme)module",
            ),
            (
                "<name>.fugr.l<name>top.reps.abap",
                "/ACME/DEMO",
                None,
                None,
                "(acme)demo.fugr.(acme)ldemotop.reps.abap",
            ),
            (
                "<name>_<name>.abap",
                "/ACME/DEMO",
                None,
                None,
                "(acme)demo_demo.abap",
            ),
            (
                "<name>.fugr.sapl<name>.reps.abap",
                "/AcMe/DeMo_\u{00c4}",
                None,
                None,
                "(acme)demo_\u{00c4}.fugr.(acme)sapldemo_\u{00c4}.reps.abap",
            ),
            (
                "<name>.<name>.<lang>.<lang>",
                "Z_EXAMPLE",
                None,
                Some("en-GB"),
                "z_example.z_example.en-GB.en-GB",
            ),
        ] {
            let mut template = FilenameTemplate::new(template);
            if parent.is_some() {
                template = template
                    .with_names(&[("name", NameSource::Parent), ("fmname", NameSource::Object)]);
            }
            assert_eq!(template.requires_parent(), parent.is_some());
            let name = ObjectName::parse(name).unwrap();
            let parent = parent.map(ObjectName::parse).transpose().unwrap();
            assert_eq!(
                template.substitute(name, parent, language).unwrap(),
                expected
            );
        }

        let child = FilenameTemplate::new("<name>.<fmname>")
            .with_names(&[("name", NameSource::Parent), ("fmname", NameSource::Object)]);
        assert!(matches!(
            child.substitute(ObjectName::parse("Z_MODULE").unwrap(), None, None),
            Err(ProjectionError::InvalidAffField {
                field: "parent",
                ..
            })
        ));
    }
}
