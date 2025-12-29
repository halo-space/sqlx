#[cfg(test)]
mod tests {
    use crate::flavor::Flavor;
    use pretty_assertions::assert_eq;

    #[test]
    fn flavor_display_like_go() {
        let cases = vec![
            (Flavor::MySQL, "MySQL"),
            (Flavor::PostgreSQL, "PostgreSQL"),
            (Flavor::SQLite, "SQLite"),
            (Flavor::SQLServer, "SQLServer"),
            (Flavor::CQL, "CQL"),
            (Flavor::ClickHouse, "ClickHouse"),
            (Flavor::Presto, "Presto"),
            (Flavor::Oracle, "Oracle"),
            (Flavor::Informix, "Informix"),
            (Flavor::Doris, "Doris"),
        ];

        for (f, expected) in cases {
            assert_eq!(f.to_string(), expected);
        }
    }
}
