use monty::{Dump, MontyRepl, MontyRun, ReplProgress, Session, SessionRef, dump};
use monty_types::{CompileOptions, MontyObject, NameLookupResult, PrintWriter, ResourceTracker};

#[test]
fn prepared_methods_and_saved_native_methods_survive_dump_restore() {
    let mut repl = MontyRepl::new("test.py", ResourceTracker::default(), CompileOptions::default());
    repl.feed_run(
        r"
items = []
append = items.append
unbound = list.append
fromkeys = dict.fromkeys
methods = {list.append: 1}
class Appender:
    def add(self, value):
        items.append(value)
app = Appender()
",
        vec![],
        PrintWriter::Disabled,
    )
    .unwrap();
    let mut progress = repl
        .feed_start(
            r"
items.append(host())
app.add(host())
append(2)
unbound(items, 3)
assert fromkeys(['a'], items) == {'a': [1, 4, 2, 3]}
assert methods[list.append] == 1
items
",
            vec![],
            PrintWriter::Disabled,
        )
        .unwrap();
    while let ReplProgress::NameLookup(lookup) = progress {
        let function = MontyObject::function(lookup.name.clone(), None);
        progress = lookup
            .resume(NameLookupResult::Value(function), PrintWriter::Disabled)
            .unwrap();
    }
    for answer in [1, 4] {
        let bytes = dump("test.py", None, &[], SessionRef::Suspended(&progress)).unwrap();
        // Abandon the original suspension so its stack references are released.
        drop(progress.into_repl());
        let loaded = Dump::load(&bytes).unwrap();
        let Session::Suspended(restored) = loaded.state else {
            panic!("expected a suspended session");
        };
        progress = restored
            .into_function_call()
            .unwrap()
            .resume(MontyObject::int(answer), PrintWriter::Disabled)
            .unwrap();
    }
    let ReplProgress::Complete { value, .. } = progress else {
        panic!("expected completion");
    };
    assert_eq!(
        value,
        MontyObject::list([
            MontyObject::int(1),
            MontyObject::int(4),
            MontyObject::int(2),
            MontyObject::int(3)
        ])
    );
}

#[test]
fn method_lookup_order_and_native_binding() {
    let mut run = MontyRun::new(
        include_str!("../test_cases/call__method_lookup.py").to_owned(),
        "test.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    assert_eq!(run.run_no_limits(vec![]).unwrap(), MontyObject::bool(true));
}
