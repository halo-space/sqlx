#[cfg(test)]
mod tests {
    use crate::SelectBuilder;
    use crate::modifiers::Builder;

    #[test]
    fn select_macro_variadic_builds_sql() {
        let mut sb = SelectBuilder::new();
        crate::select_cols!(sb, "id", "name");
        crate::from_tables!(sb, "users");
        crate::order_by_cols!(sb, "name");

        let (sql, args) = sb.build();
        assert_eq!(sql, "SELECT id, name FROM users ORDER BY name");
        assert!(args.is_empty());
    }
}
