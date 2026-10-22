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
fn registered_methods_are_available_as_values() {
    run_assertions(
        r"
upper = getattr(str, 'upper')
append = getattr(list, 'append')
count = getattr(tuple, 'count')
assert upper('abc') == 'ABC'
items = []
append(items, 1)
assert items == [1]
assert count((1, 1, 2), 1) == 2
True
",
    );
}
