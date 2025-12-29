#[cfg(test)]
mod tests {
    use crate::builder;
    use crate::modifiers::{Arg, Builder};
    use crate::{Flavor, SelectBuilder, UnionBuilder, set_default_flavor_scoped};
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn union_for_sqlite_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let sb1 = crate::builder::build(
            "SELECT id, name FROM users WHERE created_at > DATE('now', '-15 days')",
            Vec::<crate::modifiers::Arg>::new(),
        );
        let sb2 = crate::builder::build(
            "SELECT id, nick_name FROM user_extras WHERE status IN (1, 2, 3)",
            Vec::<crate::modifiers::Arg>::new(),
        );
        let mut ub = UnionBuilder::new();
        ub.union_all([sb1, sb2])
            .order_by(["id"])
            .limit(100)
            .offset(5);
        let (sql, _args) = ub.build_with_flavor(Flavor::SQLite, &[]);
        assert_eq!(
            sql,
            "SELECT id, name FROM users WHERE created_at > DATE('now', '-15 days') UNION ALL SELECT id, nick_name FROM user_extras WHERE status IN (1, 2, 3) ORDER BY id LIMIT ? OFFSET ?"
        );
    }

    #[test]
    fn union_limit_offset_matrix_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
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

        let mut ub = UnionBuilder::new();
        let save = |ub: &mut UnionBuilder, results: &mut [Vec<String>]| {
            let mut sb1 = SelectBuilder::new();
            crate::select_cols!(sb1, "*");
            crate::from_tables!(sb1, "user1");
            let mut sb2 = SelectBuilder::new();
            crate::select_cols!(sb2, "*");
            crate::from_tables!(sb2, "user2");

            ub.union([sb1, sb2]);
            for (i, f) in flavors.iter().enumerate() {
                let (s, _) = ub.build_with_flavor(*f, &[]);
                results[i].push(s);
            }
        };

        // #1
        ub.limit(-1).offset(-1);
        save(&mut ub, &mut results);
        // #2
        ub.limit(-1).offset(0);
        save(&mut ub, &mut results);
        // #3
        ub.limit(1).offset(0);
        save(&mut ub, &mut results);
        // #4
        ub.limit(1).offset(-1);
        save(&mut ub, &mut results);
        // #5
        ub.limit(1).offset(1).order_by(["id"]);
        save(&mut ub, &mut results);

        let expected = vec![
            // MySQL
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT ? OFFSET ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY id LIMIT ? OFFSET ?",
            ],
            // PostgreSQL
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) OFFSET $1",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT $1 OFFSET $2",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT $1",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY id LIMIT $1 OFFSET $2",
            ],
            // SQLite
            vec![
                "SELECT * FROM user1 UNION SELECT * FROM user2",
                "SELECT * FROM user1 UNION SELECT * FROM user2",
                "SELECT * FROM user1 UNION SELECT * FROM user2 LIMIT ? OFFSET ?",
                "SELECT * FROM user1 UNION SELECT * FROM user2 LIMIT ?",
                "SELECT * FROM user1 UNION SELECT * FROM user2 ORDER BY id LIMIT ? OFFSET ?",
            ],
            // SQLServer
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY 1 OFFSET @p1 ROWS",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY 1 OFFSET @p1 ROWS FETCH NEXT @p2 ROWS ONLY",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY 1 OFFSET 0 ROWS FETCH NEXT @p1 ROWS ONLY",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY id OFFSET @p1 ROWS FETCH NEXT @p2 ROWS ONLY",
            ],
            // CQL
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY id LIMIT ?",
            ],
            // ClickHouse
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT ? OFFSET ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY id LIMIT ? OFFSET ?",
            ],
            // Presto
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) OFFSET ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) OFFSET ? LIMIT ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT ?",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY id OFFSET ? LIMIT ?",
            ],
            // Oracle
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "SELECT * FROM ( (SELECT * FROM user1) UNION (SELECT * FROM user2) ) OFFSET :1 ROWS",
                "SELECT * FROM ( (SELECT * FROM user1) UNION (SELECT * FROM user2) ) OFFSET :1 ROWS FETCH NEXT :2 ROWS ONLY",
                "SELECT * FROM ( (SELECT * FROM user1) UNION (SELECT * FROM user2) ) OFFSET 0 ROWS FETCH NEXT :1 ROWS ONLY",
                "SELECT * FROM ( (SELECT * FROM user1) UNION (SELECT * FROM user2) ) ORDER BY id OFFSET :1 ROWS FETCH NEXT :2 ROWS ONLY",
            ],
            // Informix
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "SELECT * FROM ( (SELECT * FROM user1) UNION (SELECT * FROM user2) ) SKIP ? FIRST ?",
                "SELECT * FROM ( (SELECT * FROM user1) UNION (SELECT * FROM user2) ) FIRST ?",
                "SELECT * FROM ( (SELECT * FROM user1) UNION (SELECT * FROM user2) ) ORDER BY id SKIP ? FIRST ?",
            ],
            // Doris
            vec![
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2)",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT 1 OFFSET 0",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) LIMIT 1",
                "(SELECT * FROM user1) UNION (SELECT * FROM user2) ORDER BY id LIMIT 1 OFFSET 1",
            ],
        ];

        for (i, exp) in expected.into_iter().enumerate() {
            assert_eq!(results[i], exp);
        }
    }

    #[test]
    fn union_example_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut sb1 = SelectBuilder::new();
        let expr1 = sb1.greater_than("id", 1234);
        crate::select_cols!(sb1, "id", "name", "created_at");
        crate::from_tables!(sb1, "demo.user");
        crate::where_exprs!(sb1, expr1);

        let mut sb2 = SelectBuilder::new();
        let expr2 = sb2.in_("status", [1_i64, 2, 5]);
        crate::select_cols!(sb2, "id", "avatar");
        crate::from_tables!(sb2, "demo.user_profile");
        crate::where_exprs!(sb2, expr2);

        let mut ub = UnionBuilder::new();
        ub.union([sb1, sb2]).order_by_desc("created_at");

        let (sql, args) = ub.build();
        assert_eq!(
            sql,
            "(SELECT id, name, created_at FROM demo.user WHERE id > ?) UNION (SELECT id, avatar FROM demo.user_profile WHERE status IN (?, ?, ?)) ORDER BY created_at DESC"
        );
        assert_eq!(
            args,
            vec![
                Arg::from(1234_i64),
                Arg::from(1_i64),
                Arg::from(2_i64),
                Arg::from(5_i64),
            ]
        );
    }

    #[test]
    fn union_all_example_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut sb = SelectBuilder::new();
        let expr = sb.greater_than("id", 1234);
        crate::select_cols!(sb, "id", "name", "created_at");
        crate::from_tables!(sb, "demo.user");
        crate::where_exprs!(sb, expr);

        let mut ub = UnionBuilder::new();
        ub.union_all([
            Box::new(sb) as Box<dyn Builder>,
            builder::build("TABLE demo.user_profile", Vec::<Arg>::new()),
        ])
        .order_by_asc("created_at")
        .limit(100)
        .offset(5);

        let (sql, args) = ub.build();
        assert_eq!(
            sql,
            "(SELECT id, name, created_at FROM demo.user WHERE id > ?) UNION ALL (TABLE demo.user_profile) ORDER BY created_at ASC LIMIT ? OFFSET ?"
        );
        assert_eq!(
            args,
            vec![Arg::from(1234_i64), Arg::from(100_i64), Arg::from(5_i64)]
        );
    }

    #[test]
    fn union_builder_sql_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut sb1 = SelectBuilder::new();
        crate::select_cols!(sb1, "id", "name", "created_at");
        crate::from_tables!(sb1, "demo.user");
        let mut sb2 = SelectBuilder::new();
        crate::select_cols!(sb2, "id", "avatar");
        crate::from_tables!(sb2, "demo.user_profile");

        let mut ub = UnionBuilder::new();
        ub.sql("/* before */")
            .union([sb1, sb2])
            .sql("/* after union */");
        crate::order_by_cols!(ub, "created_at");
        ub.desc()
            .sql("/* after order by */")
            .limit(100)
            .offset(5)
            .sql("/* after limit */");

        let (sql, args) = ub.build();
        assert_eq!(
            sql,
            "/* before */ (SELECT id, name, created_at FROM demo.user) UNION (SELECT id, avatar FROM demo.user_profile) /* after union */ ORDER BY created_at DESC /* after order by */ LIMIT ? OFFSET ? /* after limit */"
        );
        assert_eq!(args, vec![Arg::from(100_i64), Arg::from(5_i64)]);
    }

    #[test]
    fn union_builder_get_flavor_like_go() {
        let mut ub = UnionBuilder::new();
        ub.set_flavor(Flavor::PostgreSQL);
        assert_eq!(ub.flavor(), Flavor::PostgreSQL);

        let mut ub_click = UnionBuilder::new();
        ub_click.set_flavor(Flavor::ClickHouse);
        assert_eq!(ub_click.flavor(), Flavor::ClickHouse);
    }

    #[test]
    fn union_builder_clone_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut sb1 = SelectBuilder::new();
        let expr3 = sb1.equal("active", 1);
        crate::select_cols!(sb1, "id");
        crate::from_tables!(sb1, "users");
        crate::where_exprs!(sb1, expr3);
        let mut sb2 = SelectBuilder::new();
        let expr4 = sb2.in_("status", [1_i64, 2]);
        crate::select_cols!(sb2, "id", "nick");
        crate::from_tables!(sb2, "profiles");
        crate::where_exprs!(sb2, expr4);

        let mut ub = UnionBuilder::new();
        ub.union_all([sb1, sb2]);
        crate::order_by_cols!(ub, "id");
        ub.desc().limit(5).offset(1);
        let mut cloned = ub.clone_builder();

        let (s1, args1) = ub.build();
        let (s2, args2) = cloned.build();
        assert_eq!(s1, s2);
        assert_eq!(args1, args2);

        cloned.asc().limit(10);
        let (sql_after, _) = cloned.build();
        let (sql_original, _) = ub.build();
        assert_ne!(sql_original, sql_after);
    }
}
