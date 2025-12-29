#[cfg(test)]
mod tests {
    use crate::create_table::CreateTableBuilder;
    use crate::modifiers::Builder;
    use pretty_assertions::assert_eq;

    #[test]
    fn create_table_example_strings() {
        let mut ctb = CreateTableBuilder::new();
        ctb.create_table("demo.user").if_not_exists();
        ctb.define([
            "id",
            "BIGINT(20)",
            "NOT NULL",
            "AUTO_INCREMENT",
            "PRIMARY KEY",
            r#"COMMENT "user id""#,
        ]);
        assert_eq!(
            ctb.build().0,
            "CREATE TABLE IF NOT EXISTS demo.user (id BIGINT(20) NOT NULL AUTO_INCREMENT PRIMARY KEY COMMENT \"user id\")"
        );

        let mut ctb = CreateTableBuilder::new();
        ctb.create_temp_table("demo.user").if_not_exists();
        ctb.define([
            "id",
            "BIGINT(20)",
            "NOT NULL",
            "AUTO_INCREMENT",
            "PRIMARY KEY",
            r#"COMMENT "user id""#,
        ]);
        assert_eq!(
            ctb.build().0,
            "CREATE TEMPORARY TABLE IF NOT EXISTS demo.user (id BIGINT(20) NOT NULL AUTO_INCREMENT PRIMARY KEY COMMENT \"user id\")"
        );
    }

    #[test]
    fn create_table_sql_and_option() {
        let mut ctb = CreateTableBuilder::new();
        ctb.create_temp_table("demo.user").if_not_exists();
        ctb.sql("/* before */");
        ctb.define(["id", "BIGINT(20)", "NOT NULL"]);
        ctb.sql("/* after define */");
        ctb.option(["DEFAULT CHARACTER SET", "utf8mb4"]);
        let (sql, args) = ctb.build();
        assert!(sql.contains("/* before */"));
        assert!(sql.contains("/* after define */"));
        assert!(sql.contains("DEFAULT CHARACTER SET utf8mb4"));
        assert!(args.is_empty());
    }

    #[test]
    fn create_table_num_define_and_clone() {
        let mut ctb = CreateTableBuilder::new();
        ctb.create_table("demo.user").if_not_exists();
        ctb.define(["id", "BIGINT(20)", "NOT NULL"]);
        ctb.define(["name", "VARCHAR(255)", "NOT NULL"]);
        assert_eq!(ctb.num_define(), 2);

        let mut clone = ctb.clone();
        clone.define(["created_at", "DATETIME", "NOT NULL"]);
        let (sql_orig, _) = ctb.build();
        let (sql_clone, _) = clone.build();
        assert!(sql_orig.contains("id BIGINT(20)"));
        assert!(sql_clone.contains("created_at DATETIME"));
    }
}
