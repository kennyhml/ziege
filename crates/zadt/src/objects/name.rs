/// A validated, borrowed ABAP object name with an optional namespace.
///
/// Both parts borrow the input and retain its spelling. Parsing does not allocate,
/// normalize case, or check object-family-specific character sets and length limits.
/// Existing keys and property models continue to store their names as strings.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObjectName<'a> {
    namespace: Option<&'a str>,
    local_name: &'a str,
}

/// A name outside the supported plain or `/NAMESPACE/NAME` syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid ABAP object name syntax")]
pub struct InvalidObjectName;

impl<'a> ObjectName<'a> {
    /// Parses a plain name or `/NAMESPACE/NAME`, preserving case and Unicode.
    ///
    /// Rejects empty names, surrounding whitespace, control characters, backslashes,
    /// angle brackets, parentheses, and the plain names `.` and `..`. Namespaced
    /// names require two nonempty parts. No additional slashes are allowed.
    /// This is structural validation, not a complete SAP naming-rule check.
    ///
    /// ```
    /// use zadt::ObjectName;
    ///
    /// let name = ObjectName::parse("/ACME/DEMO")?;
    /// assert_eq!(name.namespace(), Some("ACME"));
    /// assert_eq!(name.local_name(), "DEMO");
    /// assert_eq!(ObjectName::parse("Z_DEMO")?.namespace(), None);
    /// # Ok::<(), zadt::InvalidObjectName>(())
    /// ```
    pub fn parse(value: &'a str) -> Result<Self, InvalidObjectName> {
        if value.is_empty()
            || value.trim() != value
            || value.chars().any(char::is_control)
            || value.contains(['\\', '<', '>', '(', ')'])
            || matches!(value, "." | "..")
        {
            return Err(InvalidObjectName);
        }
        let (namespace, local_name) = if let Some(namespaced) = value.strip_prefix('/') {
            let (namespace, local_name) = namespaced.split_once('/').ok_or(InvalidObjectName)?;
            if namespace.is_empty() {
                return Err(InvalidObjectName);
            }
            (Some(namespace), local_name)
        } else {
            (None, value)
        };
        if local_name.is_empty() || local_name.contains('/') {
            return Err(InvalidObjectName);
        }
        Ok(Self {
            namespace,
            local_name,
        })
    }

    /// Returns the namespace without its slash delimiters.
    pub const fn namespace(&self) -> Option<&'a str> {
        self.namespace
    }

    /// Returns the object name without the namespace.
    pub const fn local_name(&self) -> &'a str {
        self.local_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_borrow_the_input_and_preserve_spelling() {
        let input = String::from("/Acme/Demo_\u{00c4}");
        let name = ObjectName::parse(&input).unwrap();
        assert_eq!(name.namespace(), Some("Acme"));
        assert_eq!(name.local_name(), "Demo_\u{00c4}");
        assert!(std::ptr::eq(name.namespace().unwrap(), &input[1..5]));
        assert!(std::ptr::eq(name.local_name(), &input[6..]));
        let local = ObjectName::parse(&input[6..]).unwrap();
        assert_eq!(local.namespace(), None);
        assert!(std::ptr::eq(local.local_name(), &input[6..]));
    }

    #[test]
    fn rejects_invalid_structure_without_enforcing_family_naming_rules() {
        for input in [
            "",
            " NAME",
            "NAME ",
            "\u{2003}NAME",
            "NA\nME",
            "NA\u{0085}ME",
            "NA\\ME",
            "<name>",
            "NA>ME",
            "(acme)name",
            ".",
            "..",
            "/",
            "/ACME",
            "//NAME",
            "/ACME/",
            "/ACME/NAME/EXTRA",
            "ACME/NAME",
            "/AC(ME/NAME",
            "/ACME/NA)ME",
        ] {
            assert_eq!(
                ObjectName::parse(input),
                Err(InvalidObjectName),
                "{input:?}"
            );
        }
        for input in [
            "Z_NAME",
            "/ACME/NAME",
            "Z_\u{00c4}",
            "Z.NAME",
            "Z NAME",
            "/ACME/.",
        ] {
            assert!(ObjectName::parse(input).is_ok(), "{input:?}");
        }
    }
}
