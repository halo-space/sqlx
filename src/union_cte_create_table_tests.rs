#[cfg(test)]
mod tests {
    use crate::modifiers::Builder;
    use crate::{
        CTEBuilder, CTEQueryBuilder, CreateTableBuilder, Flavor, SelectBuilder, UnionBuilder,
        create_table_define, create_table_option, cte_query_table, from_tables, select_cols,
        set_default_flavor_scoped,
    };
    use pretty_assertions::assert_eq;

    #[test]
    fn union_basic() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut sb1 = SelectBuilder::new();
        select_cols!(sb1, "id");
        from_tables!(sb1, "t1");
        let mut sb2 = SelectBuilder::new();
        select_cols!(sb2, "id");
        from_tables!(sb2, "t2");

        let mut ub = UnionBuilder::new();
        ub.union([sb1, sb2]).order_by_asc("id").limit(10);
        let (sql, _args) = ub.build();
        assert_eq!(
            sql,
            "(SELECT id FROM t1) UNION (SELECT id FROM t2) ORDER BY id ASC LIMIT ?"
        );
    }

    #[test]
    fn cte_basic() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut q = CTEQueryBuilder::new();
        let mut sb = SelectBuilder::new();
        select_cols!(sb, "id");
        from_tables!(sb, "users");
        cte_query_table!(q, "t", "id").as_(sb);

        let mut cte = CTEBuilder::new();
        cte.with([q]);
        let (sql, _args) = cte.build();
        assert_eq!(sql, "WITH t (id) AS (SELECT id FROM users)");
    }

    #[test]
    fn create_table_basic() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut ct = CreateTableBuilder::new();
        ct.create_table("users").if_not_exists();
        create_table_define!(ct, "id", "INT");
        create_table_define!(ct, "name", "TEXT");
        create_table_option!(ct, "ENGINE=InnoDB");

        let (sql, _args) = ct.build();
        assert_eq!(
            sql,
            "CREATE TABLE IF NOT EXISTS users (id INT, name TEXT) ENGINE=InnoDB"
        );
    }
}
