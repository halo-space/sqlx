#[cfg(test)]
mod tests {
    use crate::Flavor;
    use crate::cond::Cond;
    use crate::modifiers::Builder;
    use crate::modifiers::{SqlNamedArg, list};
    use crate::where_clause::{WhereClause, WhereClauseBuilder};
    use pretty_assertions::assert_eq;

    #[test]
    fn cond_in_empty_values() {
        let c = Cond::new();
        assert_eq!(c.in_("id", std::iter::empty::<i64>()), "0 = 1");
        assert_eq!(c.not_in("id", std::iter::empty::<i64>()), "0 = 0");
    }

    #[test]
    fn cond_or_and_not() {
        let c = Cond::new();
        assert_eq!(c.or(["a=1", "", "b=2"]), "(a=1 OR b=2)");
        assert_eq!(c.and(["a=1", "b=2"]), "(a=1 AND b=2)");
        assert_eq!(c.not("a=1"), "NOT a=1");
        assert_eq!(c.not(""), "");
    }

    #[test]
    fn where_clause_groups_by_args() {
        let c1 = Cond::new();
        let c2 = Cond::new();

        let wc = WhereClause::new();
        {
            let mut w = wc.borrow_mut();
            w.add_where_expr(c1.clone().args, [c1.eq("id", 1_i64)]);
            w.add_where_expr(c1.clone().args, [c1.eq("level", 2_i64)]);
            w.add_where_expr(c2.clone().args, [c2.eq("name", "foo")]);
        }

        let b = WhereClauseBuilder::new(wc);
        let (sql, _args) = b.build_with_flavor(Flavor::MySQL, &[]);
        assert_eq!(sql, "WHERE id = ? AND level = ? AND name = ?");
    }

    #[test]
    fn ilike_flavor_dependent() {
        let c = Cond::new();
        let expr = c.ilike("name", "foo@%");
        let (sql_pg, _args) = c
            .args
            .borrow()
            .compile_with_flavor(&expr, Flavor::PostgreSQL, &[]);
        assert_eq!(sql_pg, "name ILIKE $1");

        let (sql_my, _args2) = c
            .args
            .borrow()
            .compile_with_flavor(&expr, Flavor::MySQL, &[]);
        assert_eq!(sql_my, "LOWER(name) LIKE LOWER(?)");
    }

    #[test]
    fn in_list_modifier_expands() {
        let c = Cond::new();
        let expr = c.in_("status", [list([1_i64, 2, 5])]);
        let (sql, _args) = c
            .args
            .borrow()
            .compile_with_flavor(&expr, Flavor::MySQL, &[]);
        assert_eq!(sql, "status IN (?, ?, ?)");
    }

    #[test]
    fn sql_named_arg_reuses_at_name() {
        let c = Cond::new();
        let start = SqlNamedArg::new("start", 123_i64);
        let end = SqlNamedArg::new("end", 456_i64);
        let expr = c.between("created_at", start.clone(), end.clone());
        let expr2 = c.ge("modified_at", start);

        let wc = WhereClause::new();
        wc.borrow_mut()
            .add_where_expr(c.clone().args, [expr, expr2]);
        let b = WhereClauseBuilder::new(wc);
        let (sql, args) = b.build_with_flavor(Flavor::MySQL, &[]);
        assert_eq!(
            sql,
            "WHERE created_at BETWEEN @start AND @end AND modified_at >= @start"
        );
        assert_eq!(args.len(), 2);
    }
}
