#[cfg(test)]
mod tests {
    use crate::modifiers::{Arg, Builder};
    use crate::{Flavor, InsertBuilder, set_default_flavor_scoped};
    use crate::{insert_cols, returning_cols};
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn insert_values_basic() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut ib = InsertBuilder::new();
        ib.insert_into("t1");
        insert_cols!(ib, "col1", "col2").values([1_i64, 2_i64]);
        let (sql, args) = ib.build();
        assert_eq!(sql, "INSERT INTO t1 (col1, col2) VALUES (?, ?)");
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn insert_returning_postgres() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut ib = InsertBuilder::new();
        ib.insert_into("t1");
        insert_cols!(ib, "col1").values([1_i64]).returning(["id"]);
        let (sql, _args) = ib.build_with_flavor(Flavor::PostgreSQL, &[]);
        assert_eq!(sql, "INSERT INTO t1 (col1) VALUES ($1) RETURNING id");
    }

    #[test]
    fn insert_ignore_postgres_on_conflict() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut ib = InsertBuilder::new();
        ib.set_flavor(Flavor::PostgreSQL);
        insert_cols!(ib.insert_ignore_into("t1"), "col1").values([1_i64]);
        let (sql, _args) = ib.build();
        assert_eq!(
            sql,
            "INSERT INTO t1 (col1) VALUES ($1) ON CONFLICT DO NOTHING"
        );
    }

    #[test]
    fn insert_builder_returning_matrix_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut ib = InsertBuilder::new();
        ib.insert_into("user");
        insert_cols!(ib, "name").values(["Huan Du"]);
        returning_cols!(ib, "id");

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
        ];
        let expected = [
            "INSERT INTO user (name) VALUES (?)",
            "INSERT INTO user (name) VALUES ($1) RETURNING id",
            "INSERT INTO user (name) VALUES (?) RETURNING id",
            "INSERT INTO user (name) OUTPUT INSERTED.id VALUES (@p1)",
            "INSERT INTO user (name) VALUES (?)",
            "INSERT INTO user (name) VALUES (?)",
            "INSERT INTO user (name) VALUES (?)",
            "INSERT INTO user (name) VALUES (:1)",
            "INSERT INTO user (name) VALUES (?)",
        ];

        for (flavor, expected_sql) in flavors.iter().zip(expected) {
            let (sql, _args) = ib.build_with_flavor(*flavor, &[]);
            assert_eq!(sql, expected_sql);
        }
    }

    #[test]
    fn insert_builder_clone_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let mut ib = InsertBuilder::new();
        ib.insert_into("demo.user")
            .cols(["id", "name"])
            .values(vec![Arg::from(1_i64), Arg::from("A")]);

        let mut cloned = ib.clone_builder();
        let (sql1, args1) = ib.build();
        let (sql2, args2) = cloned.build();
        assert_eq!(sql1, sql2);
        assert_eq!(args1, args2);

        cloned.values(vec![Arg::from(2_i64), Arg::from("B")]);
        let (sql_after, _) = cloned.build();
        let (sql_original, _) = ib.build();
        assert_ne!(sql_original, sql_after);
    }
}
