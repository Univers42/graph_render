//! The hub contract's own tests, one file per concern.

mod batch;
mod cells;
mod change;
mod fixtures;
mod growth;
mod manifest;
mod materialize;
mod negctl;
mod property;
mod rng;
mod scalars;
mod support;
mod wire;
    #[test]
    fn dbg_model() {
        use super::fixtures::{manifest, upsert};
        use super::super::model::Model;
        use super::super::Limits;
        let mut model = Model::new("ws").unwrap();
        model.register("tracker", manifest("tracker")).unwrap();
        model
            .apply(
                "tracker",
                &upsert("task", "r1", 1, r#""name":"W","blocks":["r9"]"#),
                &Limits::DEFAULT,
            )
            .unwrap();
        println!("TEXT {}", model.to_json());
        println!("PIECES {:?}", model.pieces());
    }
}
