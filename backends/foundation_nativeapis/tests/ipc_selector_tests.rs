//! Tests extracted from shared/ipc/selector.rs
mod tests {
    use foundation_nativeapis::shared::ipc::*;
    
    use foundation_nativeapis::ipc::Label;

    #[test]
    fn unicast_validates() {
        let sel = Selector::unicast(LabelOp::Leaf("target".into()));
        assert!(sel.validate(&Label::new("target")));
        assert!(!sel.validate(&Label::new("other")));
    }

    #[test]
    fn broadcast_validates() {
        let sel = Selector::multicast(LabelOp::True);
        assert!(sel.validate(&Label::new("anything")));
        assert!(sel.validate(&Label::new("foo")));
    }

    #[test]
    fn multicast_or() {
        let sel = Selector::multicast(LabelOp::Or(
            Box::new(LabelOp::Leaf("a".into())),
            Box::new(LabelOp::Leaf("b".into())),
        ));
        assert!(sel.validate(&Label::new("a")));
        assert!(sel.validate(&Label::new("b")));
        assert!(!sel.validate(&Label::new("c")));
    }
}
