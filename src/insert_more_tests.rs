#[cfg(test)]
mod tests {
    use crate::modifiers::Builder;
    use crate::{Flavor, InsertBuilder, set_default_flavor_scoped};
    use pretty_assertions::assert_eq;

    #[test]
    fn insert_subselect_like_go_mysql_and_oracle() {
        let _g = set_default_flavor_scoped(Flavor::MySQL);

        // MySQL
        let mut ib = InsertBuilder::new();
        ib.insert_into("demo.user").cols(["id", "name"]);
        let sb = ib.select_ref(["id", "name"]);
        sb.borrow_mut().from(["demo.test"]);
        let w = sb.borrow().eq("id", 1_i64);
        sb.borrow_mut().where_([w]);
        let (sql, args) = ib.build();
        assert_eq!(
            sql,
            "INSERT INTO demo.user (id, name) SELECT id, name FROM demo.test WHERE id = ?"
        );
        assert_eq!(args.len(), 1);

        // Oracle
        let mut ib = InsertBuilder::new();
        ib.set_flavor(Flavor::Oracle);
        ib.insert_into("demo.user").cols(["id", "name"]);
        let sb = ib.select_ref(["id", "name"]);
        sb.borrow_mut().from(["demo.test"]);
        let w = sb.borrow().eq("id", 1_i64);
        sb.borrow_mut().where_([w]);
        let (sql, args) = ib.build();
        assert_eq!(
            sql,
            "INSERT INTO demo.user (id, name) SELECT id, name FROM demo.test WHERE id = :1"
        );
        assert_eq!(args.len(), 1);
    }
}
