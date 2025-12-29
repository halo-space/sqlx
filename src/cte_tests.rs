#[cfg(test)]
mod tests {
    use crate::cte::{with, with_recursive};
    use crate::cte_query::CTEQueryBuilder;
    use crate::flavor::Flavor;
    use crate::modifiers::Builder;
    use crate::select::SelectBuilder;
    use crate::union::UnionBuilder;
    use crate::{
        cte_query_table, delete_from_tables, from_tables, join_on, select_cols, update_tables,
        where_exprs,
    };
    use pretty_assertions::assert_eq;

    fn build_users_cte() -> CTEQueryBuilder {
        let mut query = CTEQueryBuilder::new();
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "id", "level");
        from_tables!(sb, "users");
        let expr = sb.greater_equal_than("level", 10);
        where_exprs!(sb, expr);
        cte_query_table!(query, "valid_users", "id", "level").as_(sb);
        query
    }

    #[test]
    fn cte_readme_with_example() {
        let mut users = CTEQueryBuilder::new();
        let mut users_sb = SelectBuilder::new();
        select_cols!(users_sb, "id", "name");
        from_tables!(users_sb, "users");
        where_exprs!(users_sb, "name IS NOT NULL");
        cte_query_table!(users, "users", "id", "name").as_(users_sb);

        let mut devices = CTEQueryBuilder::new();
        let mut devices_sb = SelectBuilder::new();
        select_cols!(devices_sb, "device_id");
        from_tables!(devices_sb, "devices");
        cte_query_table!(devices, "devices").as_(devices_sb);

        let cte = with([users, devices]);
        let mut sb = cte.select(Vec::<String>::new());
        select_cols!(sb, "users.id", "orders.id", "devices.device_id");
        from_tables!(sb, "users", "devices");
        sb.join(
            "orders",
            [
                "users.id = orders.user_id",
                "devices.device_id = orders.device_id",
            ],
        );

        let (sql, _) = sb.build();
        let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(
            normalize(&sql),
            normalize(
                "WITH users (id, name) AS (SELECT id, name FROM users WHERE name IS NOT NULL), devices AS (SELECT device_id FROM devices) SELECT users.id, orders.id, devices.device_id FROM users, devices JOIN orders ON users.id = orders.user_id AND devices.device_id = orders.device_id"
            )
        );
    }

    #[test]
    fn cte_readme_recursive_example() {
        let mut base_sb = SelectBuilder::new();
        select_cols!(base_sb, "p.id", "p.parent_id");
        from_tables!(base_sb, "accounts AS p");
        where_exprs!(base_sb, "p.id = 2");

        let mut recursive_sb = SelectBuilder::new();
        select_cols!(recursive_sb, "c.id", "c.parent_id");
        from_tables!(recursive_sb, "accounts AS c");
        join_on!(recursive_sb, "source_accounts AS sa", "c.parent_id = sa.id");

        let mut union = UnionBuilder::new();
        union.union_all([base_sb, recursive_sb]);

        let mut query = CTEQueryBuilder::new();
        cte_query_table!(query, "source_accounts", "id", "parent_id").as_(union);

        let cte = with_recursive([query]);
        let mut final_sb = cte.select(Vec::<String>::new());
        select_cols!(final_sb, "o.id", "o.date", "o.amount");
        from_tables!(final_sb, "orders AS o");
        join_on!(
            final_sb,
            "source_accounts",
            "o.account_id = source_accounts.id"
        );

        let (sql, args) = final_sb.build();
        let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(
            normalize(&sql),
            normalize(
                "WITH RECURSIVE source_accounts (id, parent_id) AS ((SELECT p.id, p.parent_id FROM accounts AS p WHERE p.id = 2) UNION ALL (SELECT c.id, c.parent_id FROM accounts AS c JOIN source_accounts AS sa ON c.parent_id = sa.id)) SELECT o.id, o.date, o.amount FROM orders AS o JOIN source_accounts ON o.account_id = source_accounts.id"
            )
        );
        assert!(args.is_empty());
    }

    #[test]
    fn cte_builder_select_like_go() {
        let query = build_users_cte();
        let cte = with([query]);
        let mut sb = cte.select(Vec::<String>::new());
        select_cols!(sb, "valid_users.id", "valid_users.level");
        from_tables!(sb, "users");
        let where_expr = sb.less_equal_than("valid_users.level", 20_i64);
        where_exprs!(sb, where_expr);

        let (sql, args) = sb.build();
        assert!(sql.starts_with("WITH valid_users"));
        assert!(sql.contains("SELECT valid_users.id, valid_users.level"));
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn cte_builder_update_matrix_like_go() {
        let mut query = CTEQueryBuilder::new();
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "user_id");
        from_tables!(sb, "vip_users");
        cte_query_table!(query, "users", "user_id").as_(sb);
        let cte = with([query]);

        let mut ub = cte.update(Vec::<String>::new());
        update_tables!(ub, "orders");
        ub.set(["orders.transport_fee = 0"]);
        let update_expr = ub.equal("users.user_id", "orders.user_id");
        where_exprs!(ub, update_expr);

        let (sql_mysql, _) = ub.build_with_flavor(Flavor::MySQL, &[]);
        println!("sql mysql debug: {}", sql_mysql);
        println!("sql mysql debug: {}", sql_mysql);
        println!("sql mysql debug: {sql_mysql}");
        println!("dbg sql mysql: {}", sql_mysql);
        assert!(sql_mysql.contains("WITH users"));
        assert!(sql_mysql.contains("UPDATE orders"));

        let (sql_pg, _) = ub
            .clone_builder()
            .build_with_flavor(Flavor::PostgreSQL, &[]);
        assert!(sql_pg.contains("WITH users"));
        assert!(sql_pg.contains("SET orders.transport_fee = 0"));
    }

    #[test]
    fn cte_builder_delete_like_go() {
        let mut query = CTEQueryBuilder::new();
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "user_id");
        from_tables!(sb, "cheaters");
        cte_query_table!(query, "users", "user_id").as_(sb);
        let cte = with([query]);

        let mut db = cte.delete_from(Vec::<String>::new());
        delete_from_tables!(db, "awards");
        let delete_expr = db.equal("users.user_id", "awards.user_id");
        where_exprs!(db, delete_expr);

        let (sql, _) = db.build_with_flavor(Flavor::MySQL, &[]);
        assert!(sql.contains("WITH users"));
        assert!(sql.contains("DELETE FROM awards"));
    }

    #[test]
    fn cte_builder_recursive_keyword() {
        let mut query = CTEQueryBuilder::new();
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "id");
        from_tables!(sb, "accounts");
        let expr = sb.equal("id", 1);
        where_exprs!(sb, expr);
        cte_query_table!(query, "rec", "id").as_(sb);

        let cte = with_recursive([query]);
        let (sql, _) = cte.build_with_flavor(Flavor::MySQL, &[]);
        assert!(sql.contains("WITH RECURSIVE"));
    }

    #[test]
    fn cte_builder_with_multiple_tables_example() {
        let mut users = CTEQueryBuilder::new();
        let mut users_sb = SelectBuilder::new();
        select_cols!(users_sb, "id", "name");
        from_tables!(users_sb, "users");
        where_exprs!(users_sb, "name IS NOT NULL");
        cte_query_table!(users, "users", "id", "name")
            .as_(users_sb)
            .add_to_table_list();

        let mut devices = CTEQueryBuilder::new();
        let mut devices_sb = SelectBuilder::new();
        select_cols!(devices_sb, "device_id");
        from_tables!(devices_sb, "devices");
        cte_query_table!(devices, "devices", "device_id").as_(devices_sb);

        let cte = with([users, devices]);
        let mut sb = cte.select(Vec::<String>::new());
        select_cols!(sb, "users.id", "orders.id", "devices.device_id");
        from_tables!(sb, "users", "devices");
        join_on!(
            sb,
            "orders",
            "users.id = orders.user_id",
            "devices.device_id = orders.device_id",
        );

        let (sql, _) = sb.build();
        assert!(sql.starts_with(
            "WITH users (id, name) AS (SELECT id, name FROM users WHERE name IS NOT NULL)"
        ));
        assert!(sql.contains("devices (device_id) AS (SELECT device_id FROM devices)"));
        assert!(sql.contains("SELECT users.id, orders.id, devices.device_id"));
        assert!(sql.contains("FROM users, devices, users JOIN orders ON users.id = orders.user_id AND devices.device_id = orders.device_id"));
    }

    #[test]
    fn cte_builder_recursive_union_example() {
        let mut base_sb = SelectBuilder::new();
        select_cols!(base_sb, "p.id", "p.parent_id");
        from_tables!(base_sb, "accounts AS p");
        where_exprs!(base_sb, "p.id = 2");

        let mut recursive_sb = SelectBuilder::new();
        select_cols!(recursive_sb, "c.id", "c.parent_id");
        from_tables!(recursive_sb, "accounts AS c");
        join_on!(recursive_sb, "source_accounts AS sa", "c.parent_id = sa.id");

        let mut union = UnionBuilder::new();
        union.union_all([base_sb, recursive_sb]);

        let mut query = CTEQueryBuilder::new();
        cte_query_table!(query, "source_accounts", "id", "parent_id")
            .as_(union)
            .add_to_table_list();

        let cte = with_recursive([query]);
        let mut final_sb = cte.select(Vec::<String>::new());
        select_cols!(final_sb, "o.id", "o.date", "o.amount");
        from_tables!(final_sb, "orders AS o");
        join_on!(
            final_sb,
            "source_accounts",
            "o.account_id = source_accounts.id"
        );

        let (sql, _) = final_sb.build();
        assert!(sql.starts_with("WITH RECURSIVE source_accounts (id, parent_id) AS"));
        assert!(sql.contains("UNION ALL"));
        assert!(sql.contains("SELECT o.id, o.date, o.amount FROM orders AS o"));
        assert!(sql.contains("JOIN source_accounts ON o.account_id = source_accounts.id"));
    }

    #[test]
    fn cte_builder_update_example() {
        let mut query = CTEQueryBuilder::new();
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "user_id");
        from_tables!(sb, "vip_users");
        cte_query_table!(query, "users", "user_id").as_(sb);

        let cte = with([query]);
        let mut ub = cte.update(Vec::<String>::new());
        update_tables!(ub, "orders");
        ub.set(["orders.transport_fee = 0"]);
        let expr = ub.equal("users.user_id", "orders.user_id");
        where_exprs!(ub, expr);

        let (sql_mysql, _) = ub.build_with_flavor(Flavor::MySQL, &[]);
        assert!(sql_mysql.starts_with("WITH users (user_id) AS (SELECT user_id FROM vip_users)"));
        assert!(sql_mysql.contains("UPDATE orders SET orders.transport_fee = 0"));
        assert!(sql_mysql.contains("WHERE users.user_id = ?"));

        let (sql_pg, _) = ub
            .clone_builder()
            .build_with_flavor(Flavor::PostgreSQL, &[]);
        assert!(sql_pg.starts_with("WITH users (user_id) AS (SELECT user_id FROM vip_users)"));
        assert!(sql_pg.contains("UPDATE orders SET orders.transport_fee = 0"));
        assert!(sql_pg.contains("WHERE users.user_id = $1"));
    }

    #[test]
    fn cte_builder_delete_example() {
        let mut query = CTEQueryBuilder::new();
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "user_id");
        from_tables!(sb, "cheaters");
        cte_query_table!(query, "users", "user_id").as_(sb);

        let cte = with([query]);
        let mut db = cte.delete_from(Vec::<String>::new());
        delete_from_tables!(db, "awards");
        let expr = db.equal("users.user_id", "awards.user_id");
        where_exprs!(db, expr);

        let (sql, _) = db.build_with_flavor(Flavor::MySQL, &[]);
        assert!(sql.starts_with("WITH users (user_id) AS (SELECT user_id FROM cheaters)"));
        assert!(sql.contains("DELETE FROM awards"));
        assert!(sql.contains("WHERE users.user_id = ?"));
    }
}
