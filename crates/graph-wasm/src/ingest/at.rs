//! Where a value sits in an ingest document (`nodes[3].kind`), written out only when a
//! refusal names it. Formatting the path up front cost eleven `format!` allocations per
//! node record and ten per edge, all of them dropped on the success path.

use std::fmt;

#[derive(Debug, Clone, Copy)]
pub(super) struct At {
    list: &'static str,
    index: Option<usize>,
    field: Option<&'static str>,
}

impl At {
    /// The document itself; it renders as the empty path.
    pub(super) const ROOT: At = At::list("");

    pub(super) const fn list(name: &'static str) -> At {
        At {
            list: name,
            index: None,
            field: None,
        }
    }

    pub(super) const fn item(self, index: usize) -> At {
        At {
            index: Some(index),
            ..self
        }
    }

    pub(super) const fn field(self, name: &'static str) -> At {
        At {
            field: Some(name),
            ..self
        }
    }
}

impl fmt::Display for At {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.list)?;
        if let Some(index) = self.index {
            write!(f, "[{index}]")?;
        }
        if let Some(field) = self.field {
            write!(f, ".{field}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::At;

    #[test]
    fn renders_the_same_dotted_path_the_eager_format_did() {
        assert_eq!(At::ROOT.to_string(), "");
        assert_eq!(At::list("version").to_string(), "version");
        assert_eq!(At::list("nodes").item(3).to_string(), "nodes[3]");
        assert_eq!(
            At::list("edges").item(0).field("child_first").to_string(),
            "edges[0].child_first"
        );
    }
}
