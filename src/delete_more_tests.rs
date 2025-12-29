#[cfg(test)]
mod tests {
    use crate::cte::with;
    use crate::cte_query::CTEQueryBuilder;
    use crate::delete::DeleteBuilder;
    use crate::flavor::Flavor;
    use crate::modifiers::{Arg, Builder};
    use crate::select::SelectBuilder;
    use crate::set_default_flavor_scoped;
    use crate::{delete_from_tables, from_tables, returning_cols, select_cols, where_exprs};
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn delete_from_limit_string_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut db = DeleteBuilder::new();
        let where_expr = db.equal("status", 1);
        delete_from_tables!(db, "demo.user");
        where_exprs!(db, where_expr);
        db.limit(10);

        let (sql, args) = db.build();
        assert_eq!(sql, "DELETE FROM demo.user WHERE status = ? LIMIT ?");
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn delete_builder_sql_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut db = DeleteBuilder::new();
        db.sql("/* before */");
        delete_from_tables!(db, "demo.user");
        db.sql("PARTITION (p0)");
        let expr = db.greater_than("id", 1234);
        db.where_([expr])
            .sql("/* after where */")
            .order_by(["id"])
            .sql("/* after order by */")
            .limit(10)
            .sql("/* after limit */");

        let (sql, args) = db.build();
        assert_eq!(
            sql,
            "/* before */ DELETE FROM demo.user PARTITION (p0) WHERE id > ? /* after where */ ORDER BY id /* after order by */ LIMIT ? /* after limit */"
        );
        assert_eq!(args, vec![Arg::from(1234_i64), Arg::from(10_i64)]);
    }

    #[test]
    fn delete_builder_with_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut query = CTEQueryBuilder::new();
        let mut inner = SelectBuilder::new();
        let name_null = inner.is_null("name");
        select_cols!(inner, "id", "name");
        from_tables!(inner, "users");
        where_exprs!(inner, name_null);
        query.table("users", Vec::<String>::new()).as_(inner);
        let cte = with([query]);

        let mut db = cte.delete_from(Vec::<String>::new());
        delete_from_tables!(db, "orders");
        where_exprs!(db, "users.id = orders.user_id");
        let (sql, _) = db.build_with_flavor(Flavor::PostgreSQL, &[]);
        assert_eq!(
            sql,
            "WITH users AS (SELECT id, name FROM users WHERE name IS NULL) DELETE FROM orders WHERE users.id = orders.user_id"
        );
    }

    #[test]
    fn delete_builder_returning_sql_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut db = DeleteBuilder::new();
        let expr = db.equal("id", 1);
        delete_from_tables!(db, "user");
        where_exprs!(db, expr);
        returning_cols!(db, "id", "name");
        db.sql("/* comment after returning */");

        let (sql, _) = db.build_with_flavor(Flavor::PostgreSQL, &[]);
        assert_eq!(
            sql,
            "DELETE FROM user WHERE id = $1 RETURNING id, name /* comment after returning */"
        );
    }

    #[test]
    fn delete_builder_clone_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut query = CTEQueryBuilder::new();
        let mut inner = SelectBuilder::new();
        select_cols!(inner, "id");
        from_tables!(inner, "to_delete");
        query.table("temp", ["id"]).as_(inner).add_to_table_list();
        let cte = with([query]);

        let mut db = cte.delete_from(Vec::<String>::new());
        delete_from_tables!(db, "target");
        where_exprs!(db, "temp.id = target.id");
        db.order_by(["id"]);
        db.asc().limit(3);
        returning_cols!(db, "id");
        let mut clone = db.clone_builder();

        let (s1, args1) = db.build_with_flavor(Flavor::PostgreSQL, &[]);
        let (s2, args2) = clone.build_with_flavor(Flavor::PostgreSQL, &[]);
        assert_eq!(s1, s2);
        assert_eq!(args1, args2);

        clone.desc().limit(5);
        let (s_cloned, _) = clone.build_with_flavor(Flavor::PostgreSQL, &[]);
        let (s_orig, _) = db.build_with_flavor(Flavor::PostgreSQL, &[]);
        assert_ne!(s_orig, s_cloned);
    }
}
