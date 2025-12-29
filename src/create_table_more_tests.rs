#[cfg(test)]
mod tests {
    use crate::modifiers::Builder;
    use crate::{CreateTableBuilder, Flavor, create_table, set_default_flavor_scoped};
    use pretty_assertions::assert_eq;

    #[test]
    fn create_table_num_define_and_clone_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);

        let mut ctb = create_table("demo.user");
        ctb.if_not_exists()
            .define([
                "id",
                "BIGINT(20)",
                "NOT NULL",
                "AUTO_INCREMENT",
                "PRIMARY KEY",
                "COMMENT \"user id\"",
            ])
            .option(["DEFAULT CHARACTER SET", "utf8mb4"]);
        assert_eq!(ctb.num_define(), 1);

        let mut ctb2 = ctb.clone_builder();
        ctb2.define(["name", "VARCHAR(255)", "NOT NULL", "COMMENT \"user name\""]);

        assert_eq!(
            ctb.build().0,
            "CREATE TABLE IF NOT EXISTS demo.user (id BIGINT(20) NOT NULL AUTO_INCREMENT PRIMARY KEY COMMENT \"user id\") DEFAULT CHARACTER SET utf8mb4"
        );
        assert_eq!(
            ctb2.build().0,
            "CREATE TABLE IF NOT EXISTS demo.user (id BIGINT(20) NOT NULL AUTO_INCREMENT PRIMARY KEY COMMENT \"user id\", name VARCHAR(255) NOT NULL COMMENT \"user name\") DEFAULT CHARACTER SET utf8mb4"
        );
    }

    #[test]
    fn create_table_sql_and_var_like_go() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);

        let mut ctb = CreateTableBuilder::new();
        ctb.sql("/* before */");
        ctb.create_temp_table("demo.user").if_not_exists();
        ctb.sql("/* after create */");
        ctb.define([
            "id",
            "BIGINT(20)",
            "NOT NULL",
            "AUTO_INCREMENT",
            "PRIMARY KEY",
            "COMMENT \"user id\"",
        ]);
        ctb.define(["name", "VARCHAR(255)", "NOT NULL", "COMMENT \"user name\""]);
        ctb.sql("/* after define */");
        ctb.option(["DEFAULT CHARACTER SET", "utf8mb4"]);

        let tail = ctb.var(crate::builder::build(
            "AS SELECT * FROM old.user WHERE name LIKE $?",
            vec![crate::modifiers::Arg::from("%Huan%".to_string())],
        ));
        ctb.sql(tail);

        let (sql, args) = ctb.build();
        assert_eq!(
            sql,
            "/* before */ CREATE TEMPORARY TABLE IF NOT EXISTS demo.user /* after create */ (id BIGINT(20) NOT NULL AUTO_INCREMENT PRIMARY KEY COMMENT \"user id\", name VARCHAR(255) NOT NULL COMMENT \"user name\") /* after define */ DEFAULT CHARACTER SET utf8mb4 AS SELECT * FROM old.user WHERE name LIKE ?"
        );
        assert_eq!(args.len(), 1);
    }
}
