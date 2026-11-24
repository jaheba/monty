use monty::MontyRun;
use monty_types::{BuiltinsFunctions, CompileOptions, ExcType, MontyObject, Shard, Shards};

#[test]
fn builtin_values_supplied_by_the_host_obey_the_session_shards() {
    let options = CompileOptions {
        shards: Shards::default().without(Shard::DynamicEvaluation),
        ..CompileOptions::default()
    };

    for builtin in [BuiltinsFunctions::Eval, BuiltinsFunctions::Exec] {
        let mut run = MontyRun::new("f('42')".to_owned(), "test.py", vec!["f".to_owned()], options).unwrap();
        let err = run
            .run_no_limits(vec![MontyObject::builtin_function(builtin)])
            .unwrap_err();
        assert_eq!(err.exc_type(), ExcType::PermissionError);
    }
}
