//! SQL 占位符方言支持。

/// SQL 占位符风格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// 使用 `?` 作为占位符（常见于 MySQL/SQLite）。
    QuestionMark,
    /// 使用 `$1, $2, ...` 作为占位符（常见于 PostgreSQL）。
    DollarNumbered,
}

impl Dialect {
    #[allow(dead_code)]
    pub(crate) fn write_placeholder(self, index_1_based: usize, out: &mut String) {
        match self {
            Self::QuestionMark => out.push('?'),
            Self::DollarNumbered => {
                out.push('$');
                out.push_str(&index_1_based.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Dialect;

    #[test]
    fn write_placeholder_question_mark() {
        let mut s = String::new();
        Dialect::QuestionMark.write_placeholder(1, &mut s);
        assert_eq!(s, "?");
    }

    #[test]
    fn write_placeholder_dollar_numbered() {
        let mut s = String::new();
        Dialect::DollarNumbered.write_placeholder(12, &mut s);
        assert_eq!(s, "$12");
    }
}
