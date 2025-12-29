#[cfg(test)]
mod tests {
    use crate::builder::build;
    use crate::flavor::Flavor;
    use crate::modifiers::{flatten, tuple};
    use pretty_assertions::assert_eq;

    #[test]
    fn flatten_like_go_subset() {
        assert_eq!(flatten("foo"), vec!["foo".into()]);
        assert_eq!(
            flatten(vec![1_i64, 2, 3]),
            vec![1_i64.into(), 2_i64.into(), 3_i64.into()]
        );
        assert_eq!(
            flatten([1_i64, 2, 3]),
            vec![1_i64.into(), 2_i64.into(), 3_i64.into()]
        );

        // 递归展开（同类型嵌套）：
        let nested: Vec<Vec<i64>> = vec![vec![1, 2], vec![3]];
        assert_eq!(
            flatten(nested),
            vec![1_i64.into(), 2_i64.into(), 3_i64.into()]
        );
    }

    #[test]
    fn tuple_interpolate_like_go() {
        let inner: crate::modifiers::Arg = tuple::<Vec<crate::modifiers::Arg>>(vec![
            crate::modifiers::Arg::from(2_i64),
            crate::modifiers::Arg::from("baz".to_string()),
        ]);
        let outer: crate::modifiers::Arg = tuple::<Vec<crate::modifiers::Arg>>(vec![
            crate::modifiers::Arg::from("foo".to_string()),
            inner,
        ]);
        let top: crate::modifiers::Arg = tuple::<Vec<crate::modifiers::Arg>>(vec![
            crate::modifiers::Arg::from(1_i64),
            crate::modifiers::Arg::from("bar".to_string()),
            crate::modifiers::Arg::from(Option::<i64>::None),
            outer,
        ]);

        let b = build("$?", [top]);
        let (sql, args) = b.build();
        let actual = Flavor::MySQL.interpolate(&sql, &args).unwrap();
        assert_eq!(actual, "(1, 'bar', NULL, ('foo', (2, 'baz')))");
    }
}
