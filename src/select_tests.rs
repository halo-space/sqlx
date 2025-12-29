#[cfg(test)]
mod tests {
    use crate::modifiers::Builder;
    use crate::{Cond, Flavor, SelectBuilder, from_tables, select_cols, where_exprs};
    use pretty_assertions::assert_eq;

    #[test]
    fn select_basic_where_in_or() {
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "id", "name", "COUNT(*) AS c");
        from_tables!(sb, "user");

        let c: &Cond = &sb;
        where_exprs!(
            sb,
            c.in_("status", [1_i64, 2, 5]),
            c.or([c.equal("name", "foo"), c.like("email", "foo@%")]),
        );

        let (sql, args) = sb.build_with_flavor(Flavor::MySQL, &[]);
        assert_eq!(
            sql,
            "SELECT id, name, COUNT(*) AS c FROM user WHERE status IN (?, ?, ?) AND (name = ? OR email LIKE ?)"
        );
        assert_eq!(args.len(), 5);
    }

    #[test]
    fn select_order_by_limit_offset() {
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "id", "name", "score");
        from_tables!(sb, "users");
        sb.order_by_desc("score")
            .order_by_asc("name")
            .limit(10)
            .offset(20);

        let (sql, _args) = sb.build_with_flavor(Flavor::MySQL, &[]);
        assert_eq!(
            sql,
            "SELECT id, name, score FROM users ORDER BY score DESC, name ASC LIMIT ? OFFSET ?"
        );
    }
}
