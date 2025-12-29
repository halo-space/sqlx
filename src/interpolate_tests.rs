#[cfg(test)]
mod tests {
    use crate::modifiers::Arg;
    use crate::value::{SqlDateTime, SqlValue};
    use crate::{Flavor, set_default_flavor_scoped};
    use pretty_assertions::assert_eq;
    use time::UtcOffset;
    use time::macros::datetime;

    #[test]
    fn mysql_interpolate_question_marks() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let sql = "SELECT * FROM a WHERE name = ? AND state IN (?, ?)";
        let args = vec![
            Arg::Value(SqlValue::from("I'm fine")),
            Arg::Value(SqlValue::I64(42)),
            Arg::Value(SqlValue::I64(8)),
        ];
        let q = Flavor::MySQL.interpolate(sql, &args).unwrap();
        assert_eq!(
            q,
            "SELECT * FROM a WHERE name = 'I\\'m fine' AND state IN (42, 8)"
        );
    }

    #[test]
    fn postgres_interpolate_dollar_numbered_and_dollar_quote() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let sql = "SELECT $1, $2 FROM $abc$$1$abc$ WHERE x = $2";
        let args = vec![
            Arg::Value(SqlValue::I64(1)),
            Arg::Value(SqlValue::from("hi")),
        ];
        let q = Flavor::PostgreSQL.interpolate(sql, &args).unwrap();
        assert_eq!(q, "SELECT 1, E'hi' FROM $abc$$1$abc$ WHERE x = E'hi'");
    }

    #[test]
    fn sqlserver_interpolate_at_pn() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let sql = "SELECT * FROM a WHERE name = @p1 AND id = @P2";
        let args = vec![
            Arg::Value(SqlValue::from("x")),
            Arg::Value(SqlValue::I64(7)),
        ];
        let q = Flavor::SQLServer.interpolate(sql, &args).unwrap();
        assert_eq!(q, "SELECT * FROM a WHERE name = N'x' AND id = 7");
    }

    #[test]
    fn oracle_interpolate_colon_numbered_and_colon_quote() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let sql = "SELECT :1 FROM :abc::1:abc: WHERE y = :1";
        let args = vec![Arg::Value(SqlValue::I64(42))];
        let q = Flavor::Oracle.interpolate(sql, &args).unwrap();
        assert_eq!(q, "SELECT 42 FROM :abc::1:abc: WHERE y = 42");
    }

    #[test]
    fn datetime_formats_mysql_and_postgres() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);
        let dt = datetime!(2019-04-24 12:23:34.123456789)
            .assume_offset(UtcOffset::from_hms(8, 0, 0).unwrap());
        let v = SqlDateTime::new(dt).with_tz_abbr("CST");
        let args = vec![Arg::Value(SqlValue::DateTime(v))];

        let q1 = Flavor::MySQL.interpolate("SELECT ?", &args).unwrap();
        assert_eq!(q1, "SELECT '2019-04-24 12:23:34.123457'");

        let q2 = Flavor::PostgreSQL.interpolate("SELECT $1", &args).unwrap();
        assert_eq!(q2, "SELECT '2019-04-24 12:23:34.123457 CST'");
    }
}
