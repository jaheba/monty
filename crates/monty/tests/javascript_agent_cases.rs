#![cfg(feature = "javascript")]

use monty::MontyRepl;
use monty_types::{CompileOptions, MontyObject, NamedValues, PrintWriter, ResourceTracker};

fn run(source: &str) -> MontyObject {
    let mut repl = MontyRepl::new("agent.js", ResourceTracker::default(), CompileOptions::default());
    repl.feed_javascript(source, NamedValues::new(), PrintWriter::Disabled)
        .unwrap_or_else(|error| panic!("{source}\n{error}"))
}

#[test]
fn representative_data_processing_programs() {
    let cases = [
        (
            "filter and project records",
            r"
            const users = [{name: 'Ada', active: true}, {name: 'Bob', active: false}];
            const names = users.filter(u => u.active).map(u => u.name);
            JSON.stringify(names)
        ",
            r#"["Ada"]"#,
        ),
        (
            "group and count",
            r"
            const counts = {};
            for (const name of ['a', 'b', 'a']) { counts[name] = (counts[name] ?? 0) + 1; }
            JSON.stringify(counts)
        ",
            r#"{"a":2,"b":1}"#,
        ),
        (
            "nested destructuring and object spread",
            r"
            const item = {id: 7, profile: {name: 'Ada'}, active: true};
            const {profile: {name}, ...rest} = item;
            JSON.stringify({...rest, name})
        ",
            r#"{"id":7,"active":true,"name":"Ada"}"#,
        ),
        (
            "array destructuring, rest and spread calls",
            r"
            function sum(first, ...rest) { return rest.reduce((a, b) => a + b, first); }
            const [first, , ...tail] = [1, 2, 3, 4];
            JSON.stringify([first, sum(...tail)])
        ",
            "[1,7]",
        ),
        (
            "block callbacks and captured state",
            r"
            function project(offset) {
                return [1, 2, 3].map((value, index) => { const adjusted = value + offset; return adjusted + index; });
            }
            JSON.stringify(project(10))
        ",
            "[11,13,15]",
        ),
        (
            "numeric comparator",
            r"
            const data = [{n: 12}, {n: 2}, {n: 7}];
            JSON.stringify(data.sort((a, b) => a.n - b.n).map(x => x.n))
        ",
            "[2,7,12]",
        ),
        (
            "text cleanup",
            r"
            const raw = ' Ada, BOB, ,Cara ';
            const names = raw.split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
            names.join('|')
        ",
            "ada|bob|cara",
        ),
        (
            "template strings and coercing addition",
            r"
            const count = 3;
            `Found ${count} items: ${true}` + ' / ' + (null + 1)
        ",
            "Found 3 items: true / 1",
        ),
        (
            "JSON round trip",
            r#"
            const input = JSON.parse('{"values":[1,2,3]}');
            input.total = input.values.reduce((total, value) => total + value, 0);
            JSON.stringify(input)
        "#,
            r#"{"values":[1,2,3],"total":6}"#,
        ),
        (
            "enumerate and rebuild",
            r"
            const scores = {a: 3, b: 5};
            const doubled = Object.entries(scores).map(([key, value]) => [key, value * 2]);
            JSON.stringify(Object.fromEntries(doubled))
        ",
            r#"{"a":6,"b":10}"#,
        ),
        (
            "classic for and continue",
            r"
            let total = 0;
            for (let i = 0; i < 6; i++) { if (i % 2 === 0) { continue; } total += i; }
            String(total)
        ",
            "9",
        ),
        (
            "query array",
            r"
            const nums = [1, 4, 9];
            JSON.stringify([nums.find(n => n > 3), nums.some(n => n > 8), nums.every(n => n > 0), nums.includes(4)])
        ",
            "[4,true,true,true]",
        ),
        (
            "optional data and fallback",
            r"
            const response = {user: null};
            const name = response.user?.name ?? 'unknown';
            JSON.stringify([name, response.absent ?? 42, 0 ?? 7])
        ",
            r#"["unknown",42,0]"#,
        ),
        (
            "default parameters evaluate per call",
            r"
            function append(value, list = []) { list.push(value); return list; }
            JSON.stringify([append(1), append(2)])
        ",
            "[[1],[2]]",
        ),
        (
            "object mutations and array stack",
            r"
            const state = {count: 1}; state.count += 2;
            const pending = [1]; const length = pending.push(2, 3);
            JSON.stringify([state.count, length, pending.pop(), pending.slice(0, 1)])
        ",
            "[3,3,3,[1]]",
        ),
        (
            "normalize primitive values",
            r"
            JSON.stringify(['12', '3', '7'].map(Number).map(n => Math.min(n * 2, 20)))
        ",
            "[20,6,14]",
        ),
        (
            "compose function expressions",
            r"
            const scale = function (n) { return n * 2; };
            JSON.stringify([1, 2].map(scale).concat([9]).reverse())
        ",
            "[9,4,2]",
        ),
        (
            "word frequency report",
            r"
            const counts = {}; const words = 'cat dog cat'.split(' ');
            words.forEach(word => { counts[word] = (counts[word] ?? 0) + 1; });
            JSON.stringify(Object.keys(counts).sort().map(key => `${key}:${counts[key]}`))
        ",
            r#"["cat:2","dog:1"]"#,
        ),
        (
            "destructuring default",
            r"
            function label({name = 'unknown', count = 0}) { return `${name}:${count}`; }
            label({name: 'Ada'})
        ",
            "Ada:0",
        ),
        (
            "flat, string search and replacement",
            r"
            const values = [[1, 2], [3]].flat();
            JSON.stringify([values.length, 'a-b-a'.replaceAll('a', 'x'), 'hello'.startsWith('he')])
        ",
            r#"[3,"x-b-x",true]"#,
        ),
        (
            "write enclosing global binding",
            r"
            let total = 0;
            [1, 2, 3].forEach(value => { total += value; });
            String(total)
        ",
            "6",
        ),
        (
            "write captured function binding",
            r"
            function count() {
                let total = 10;
                const add = value => { total += value; return total; };
                return [1, 2].map(add);
            }
            JSON.stringify(count())
        ",
            "[11,13]",
        ),
        (
            "error recovery and finally",
            r"
            let status = '';
            try { throw new Error('bad input'); }
            catch (error) { status = error.message; }
            finally { status += '!'; }
            status
        ",
            "bad input!",
        ),
        (
            "deduplicate without losing insertion order",
            r"
            const unique = new Set(['b', 'a', 'b']);
            unique.add('c');
            JSON.stringify([[...unique], unique.has('b'), unique.size])
        ",
            "[[\"b\",\"a\",\"c\"],true,3]",
        ),
        (
            "maps with object identity keys",
            r"
            const key = {}; const table = new Map([[key, 1]]);
            table.set(key, 2).set({}, 3);
            JSON.stringify([table.get(key), table.size, table.has({}), [...table.values()]])
        ",
            "[2,2,false,[2,3]]",
        ),
        (
            "construct and populate arrays",
            r"
            const data = Array.from(new Array(3), (_, i) => i + 1);
            JSON.stringify([data, Array.isArray(data), typeof data[0]])
        ",
            "[[1,2,3],true,\"number\"]",
        ),
        (
            "finite JSON numbers",
            r"
            JSON.stringify([1 / 2, Number('bad'), Math.pow(2, 3), Number.isInteger(3), (2.5).toFixed(2)])
        ",
            "[0.5,null,8,true,\"2.50\"]",
        ),
    ];
    for (name, source, expected) in cases {
        assert_eq!(run(source), MontyObject::string(expected), "{name}");
    }
}
