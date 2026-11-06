use monty::MontyRun;
use monty_types::{CompileOptions, MontyObject};

fn run_assertions(code: &str) {
    let mut run = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    assert_eq!(run.run_no_limits(vec![]).unwrap(), MontyObject::bool(true));
}

#[test]
fn direct_calls_support_registered_methods_and_native_subclasses() {
    run_assertions(include_str!("../test_cases/builtin__unbound_methods.py"));
}

#[test]
fn direct_calls_do_not_expose_intermediate_callables() {
    run_assertions(
        r"
for owner, name in [(str, 'upper'), (list, 'append'), (tuple, 'count')]:
    try:
        getattr(owner, name)
    except AttributeError:
        pass
    else:
        raise AssertionError('method unexpectedly exposed as a value')
True
",
    );
}

#[test]
fn collection_dunders_share_native_protocols() {
    run_assertions(include_str!("../test_cases/builtin__dunder_methods.py"));
}

#[test]
fn explicit_native_protocols_preserve_one_sided_results() {
    run_assertions(include_str!("../test_cases/builtin__protocol_methods.py"));
}

#[test]
fn mutable_protocols_preserve_identity_and_iteration() {
    run_assertions(include_str!("../test_cases/builtin__mutable_protocols.py"));
}
