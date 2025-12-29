//! SQL 表达式片段。

use crate::dialect::Dialect;
use crate::value::SqlValue;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Part {
    Sql(String),
    Arg(SqlValue),
}

/// 一个可组合的 SQL 片段表达式。
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub(crate) parts: Vec<Part>,
}

impl Expr {
    /// 直接插入一段 SQL 文本（不会变成参数）。
    pub fn raw(sql: impl Into<String>) -> Self {
        Self {
            parts: vec![Part::Sql(sql.into())],
        }
    }

    /// 创建一个恒为 TRUE 的表达式（`TRUE`）。
    pub fn true_() -> Self {
        Self::raw("TRUE")
    }

    /// 创建一个恒为 FALSE 的表达式（`FALSE`）。
    pub fn false_() -> Self {
        Self::raw("FALSE")
    }

    /// 追加 SQL 文本。
    pub fn push_raw(&mut self, sql: impl Into<String>) {
        self.parts.push(Part::Sql(sql.into()));
    }

    /// 追加一个参数（构建时会生成占位符）。
    pub fn push_arg(&mut self, v: impl Into<SqlValue>) {
        self.parts.push(Part::Arg(v.into()));
    }

    /// 将当前表达式与另一个表达式连接（不自动添加空格）。
    pub fn concat(mut self, other: Expr) -> Self {
        self.parts.extend(other.parts);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn build(&self, dialect: Dialect) -> (String, Vec<SqlValue>) {
        let mut sql = String::new();
        let mut args = Vec::new();

        for part in &self.parts {
            match part {
                Part::Sql(s) => sql.push_str(s),
                Part::Arg(v) => {
                    let idx = args.len() + 1;
                    dialect.write_placeholder(idx, &mut sql);
                    args.push(v.clone());
                }
            }
        }

        (sql, args)
    }
}

#[cfg(test)]
mod tests {
    use super::Expr;
    use crate::Dialect;
    use crate::SqlValue;

    #[test]
    fn raw_is_not_parameterized() {
        let e = Expr::raw("a = 1");
        let (sql, args) = e.build(Dialect::QuestionMark);
        assert_eq!(sql, "a = 1");
        assert!(args.is_empty());
    }

    #[test]
    fn push_arg_generates_placeholder_question_mark() {
        let mut e = Expr::raw("id = ");
        e.push_arg(7_i64);
        let (sql, args) = e.build(Dialect::QuestionMark);
        assert_eq!(sql, "id = ?");
        assert_eq!(args, vec![SqlValue::I64(7)]);
    }

    #[test]
    fn push_arg_generates_placeholder_dollar_numbered() {
        let mut e = Expr::raw("id = ");
        e.push_arg(7_i64);
        let (sql, _args) = e.build(Dialect::DollarNumbered);
        assert_eq!(sql, "id = $1");
    }

    #[test]
    fn concat_keeps_arg_order() {
        let mut a = Expr::raw("a = ");
        a.push_arg(1_i64);
        let mut b = Expr::raw(" AND b = ");
        b.push_arg(2_i64);

        let e = a.concat(b);
        let (sql, args) = e.build(Dialect::QuestionMark);
        assert_eq!(sql, "a = ? AND b = ?");
        assert_eq!(args, vec![SqlValue::I64(1), SqlValue::I64(2)]);
    }
}
