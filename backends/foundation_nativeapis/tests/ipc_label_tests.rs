//! Tests extracted from shared/ipc/label.rs
mod tests {
    use foundation_nativeapis::shared::ipc::*;
    use foundation_nativeapis::shared::ipc::label::*;

    #[test]
    fn leaf_matches() {
        assert!(LabelOp::Leaf("foo".into()).matches("foo"));
        assert!(!LabelOp::Leaf("foo".into()).matches("bar"));
    }

    #[test]
    fn true_false() {
        assert!(LabelOp::True.matches("anything"));
        assert!(!LabelOp::False.matches("anything"));
    }

    #[test]
    fn not() {
        let expr = LabelOp::Not(Box::new(LabelOp::Leaf("foo".into())));
        assert!(!expr.matches("foo"));
        assert!(expr.matches("bar"));
    }

    #[test]
    fn or() {
        let expr = LabelOp::Or(
            Box::new(LabelOp::Leaf("a".into())),
            Box::new(LabelOp::Leaf("b".into())),
        );
        assert!(expr.matches("a"));
        assert!(expr.matches("b"));
        assert!(!expr.matches("c"));
    }

    #[test]
    fn label_new() {
        let label = Label::new("test");
        assert_eq!(label.as_str(), "test");
    }

    #[test]
    fn label_from_string() {
        let label: Label = "hello".into();
        assert_eq!(label.as_str(), "hello");
    }

    #[test]
    fn label_display() {
        let label = Label::new("test-label");
        assert_eq!(format!("{label}"), "test-label");
    }

    #[test]
    fn builder_and() {
        let op = LabelOp::from("a").and("b");
        assert!(!op.matches("a"));
        assert!(!op.matches("b"));
    }

    #[test]
    fn builder_or() {
        let op = LabelOp::from("a").or("b");
        assert!(op.matches("a"));
        assert!(op.matches("b"));
        assert!(!op.matches("c"));
    }

    #[test]
    fn builder_not() {
        let op = !LabelOp::from("a");
        assert!(!op.matches("a"));
        assert!(op.matches("b"));
    }

    #[test]
    fn builder_chain() {
        let op = LabelOp::from("x").or("y").and(!LabelOp::from("z"));
        assert!(op.matches("x"));
        assert!(op.matches("y"));
        assert!(!op.matches("z"));
    }
}
