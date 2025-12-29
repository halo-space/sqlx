#[cfg(test)]
mod tests {
    use crate::flavor::Flavor;
    use crate::modifiers::{Arg, Builder, flatten};
    use crate::select::SelectBuilder;
    use crate::{from_tables, join_on, order_by_cols, select_cols, where_exprs};

    type SelectCase = Box<dyn Fn(&mut SelectBuilder)>;

    #[test]
    fn select_builder_builder_as_and_flatten() {
        let mut sb = SelectBuilder::new();
        let mut inner = SelectBuilder::new();
        select_cols!(inner, "id");
        from_tables!(inner, "banned");
        let inner_expr = inner.greater_than("id", 10);
        where_exprs!(inner, inner_expr);

        let sub_alias = sb.builder_as(inner, "b");

        select_cols!(sb, "u.id", "u.name");
        from_tables!(sb, sub_alias);
        let in_expr = sb.in_("status", flatten(vec![1_i64, 2, 3]));
        where_exprs!(sb, in_expr);

        let (sql, args) = sb.build_with_flavor(Flavor::MySQL, &[]);
        assert!(sql.contains("FROM (SELECT"));
        assert!(sql.contains("AS b"));
        assert!(sql.contains("status IN (?, ?, ?)"));
        assert_eq!(args.len(), 4);
    }

    #[test]
    fn nested_select_example_like_readme() {
        let mut sb = SelectBuilder::new();

        let mut from_sb = SelectBuilder::new();
        select_cols!(from_sb, "id");
        from_tables!(from_sb, "user");
        where_exprs!(from_sb, from_sb.greater_than("level", 4_i64));

        let mut status_sb = SelectBuilder::new();
        select_cols!(status_sb, "status");
        from_tables!(status_sb, "config");
        where_exprs!(status_sb, status_sb.equal("state", 1_i64));

        let from_alias = sb.builder_as(from_sb, "user");
        select_cols!(sb, "id");
        from_tables!(sb, from_alias);
        let status_expr = sb.in_("status", vec![Arg::Builder(Box::new(status_sb))]);
        where_exprs!(sb, status_expr);

        let (sql, args) = sb.build_with_flavor(Flavor::MySQL, &[]);
        assert_eq!(
            sql,
            "SELECT id FROM (SELECT id FROM user WHERE level > ?) AS user WHERE status IN (SELECT status FROM config WHERE state = ?)"
        );
        assert_eq!(args, vec![Arg::from(4_i64), Arg::from(1_i64)],);
    }

    #[test]
    fn nested_join_example_like_readme() {
        let mut sb = SelectBuilder::new();

        let mut nested = SelectBuilder::new();
        select_cols!(nested, "b.id", "b.user_id");
        from_tables!(nested, "users2 AS b");
        let nested_expr = nested.greater_than("b.age", 20_i64);
        where_exprs!(nested, nested_expr);

        select_cols!(sb, "a.id", "a.user_id");
        from_tables!(sb, "users AS a");
        let nested_alias = sb.builder_as(nested, "b");
        join_on!(sb, nested_alias, "a.user_id = b.user_id");

        let (sql, args) = sb.build_with_flavor(Flavor::MySQL, &[]);
        assert_eq!(
            sql,
            "SELECT a.id, a.user_id FROM users AS a JOIN (SELECT b.id, b.user_id FROM users2 AS b WHERE b.age > ?) AS b ON a.user_id = b.user_id"
        );
        assert_eq!(args, vec![Arg::from(20_i64)]);
    }

    #[test]
    fn select_builder_limit_offset_matrix_like_go() {
        let flavors = [
            Flavor::MySQL,
            Flavor::PostgreSQL,
            Flavor::SQLite,
            Flavor::SQLServer,
            Flavor::CQL,
            Flavor::ClickHouse,
            Flavor::Presto,
            Flavor::Oracle,
            Flavor::Informix,
            Flavor::Doris,
        ];

        let mut results: Vec<Vec<String>> = vec![Vec::new(); flavors.len()];
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "*");
        from_tables!(sb, "user");

        let cases: Vec<SelectCase> = vec![
            Box::new(|sb| {
                sb.limit(-1);
                sb.offset(-1);
            }),
            Box::new(|sb| {
                sb.limit(-1);
                sb.offset(0);
            }),
            Box::new(|sb| {
                sb.limit(1);
                sb.offset(0);
            }),
            Box::new(|sb| {
                sb.limit(1);
                sb.offset(-1);
            }),
            Box::new(|sb| {
                sb.limit(1);
                sb.offset(1);
                order_by_cols!(sb, "id");
            }),
        ];

        for case in cases {
            case(&mut sb);
            for (i, flavor) in flavors.iter().enumerate() {
                let (sql, _) = sb.build_with_flavor(*flavor, &[]);
                results[i].push(sql);
            }
        }

        let expected = vec![
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user",
                "SELECT * FROM user LIMIT ? OFFSET ?",
                "SELECT * FROM user LIMIT ?",
                "SELECT * FROM user ORDER BY id LIMIT ? OFFSET ?",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user OFFSET $1",
                "SELECT * FROM user LIMIT $1 OFFSET $2",
                "SELECT * FROM user LIMIT $1",
                "SELECT * FROM user ORDER BY id LIMIT $1 OFFSET $2",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user",
                "SELECT * FROM user LIMIT ? OFFSET ?",
                "SELECT * FROM user LIMIT ?",
                "SELECT * FROM user ORDER BY id LIMIT ? OFFSET ?",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user ORDER BY 1 OFFSET @p1 ROWS",
                "SELECT * FROM user ORDER BY 1 OFFSET @p1 ROWS FETCH NEXT @p2 ROWS ONLY",
                "SELECT * FROM user ORDER BY 1 OFFSET 0 ROWS FETCH NEXT @p1 ROWS ONLY",
                "SELECT * FROM user ORDER BY id OFFSET @p1 ROWS FETCH NEXT @p2 ROWS ONLY",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user",
                "SELECT * FROM user LIMIT ?",
                "SELECT * FROM user LIMIT ?",
                "SELECT * FROM user ORDER BY id LIMIT ?",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user",
                "SELECT * FROM user LIMIT ? OFFSET ?",
                "SELECT * FROM user LIMIT ?",
                "SELECT * FROM user ORDER BY id LIMIT ? OFFSET ?",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user OFFSET ?",
                "SELECT * FROM user OFFSET ? LIMIT ?",
                "SELECT * FROM user LIMIT ?",
                "SELECT * FROM user ORDER BY id OFFSET ? LIMIT ?",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user OFFSET :1 ROWS",
                "SELECT * FROM user OFFSET :1 ROWS FETCH NEXT :2 ROWS ONLY",
                "SELECT * FROM user OFFSET 0 ROWS FETCH NEXT :1 ROWS ONLY",
                "SELECT * FROM user ORDER BY id OFFSET :1 ROWS FETCH NEXT :2 ROWS ONLY",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user",
                "SELECT * FROM user LIMIT ? OFFSET ?",
                "SELECT * FROM user LIMIT ?",
                "SELECT * FROM user ORDER BY id LIMIT ? OFFSET ?",
            ],
            vec![
                "SELECT * FROM user",
                "SELECT * FROM user",
                "SELECT * FROM user LIMIT ? OFFSET ?",
                "SELECT * FROM user LIMIT ?",
                "SELECT * FROM user ORDER BY id LIMIT ? OFFSET ?",
            ],
        ];

        assert_eq!(results, expected);
    }
}
