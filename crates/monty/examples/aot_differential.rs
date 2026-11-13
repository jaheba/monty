//! Reproducible typed-control-flow stress against the ordinary interpreter.

use std::{
    env,
    fmt::Write,
    fs, process,
    time::{Duration, Instant},
};

use monty::{
    MontyRun,
    experimental_aot::{self, Backend},
};
use monty_types::{CompileOptions, MontyObject, PrintWriter, ResourceLimits, ResourceTracker};

#[global_allocator]
static ALLOC: monty_alloc::LimitedAllocator = monty_alloc::LimitedAllocator;

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
    fn pick(&mut self, count: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(count).unwrap()).unwrap()
    }
    fn small(&mut self) -> i64 {
        i64::try_from(self.next() % 101).unwrap() - 50
    }
    fn expression(&mut self, depth: usize, variables: &[&str]) -> String {
        if depth == 0 || self.pick(4) == 0 {
            return match self.pick(5) {
                0 => self.small().to_string(),
                1 => format!("data[{}]", i32::try_from(self.pick(7)).unwrap() - 3),
                2 => "row['k']".to_owned(),
                _ => variables[self.pick(variables.len())].to_owned(),
            };
        }
        let left = self.expression(depth - 1, variables);
        let right = self.expression(depth - 1, variables);
        let op = ["+", "-", "*", "//", "%", "&", "|", "^"][self.pick(8)];
        format!("({left} {op} {right})")
    }
    fn update(&mut self, variables: &[&str]) -> String {
        let depth = 1 + self.pick(3);
        let expression = self.expression(depth, variables);
        format!("({expression}) % 1000003")
    }
    fn floats(&mut self, case: u64) -> String {
        let x = [
            "0.0",
            "-0.0",
            "1.25",
            "-3.5",
            "1e308",
            "1e-308",
            "float('nan')",
            "float('inf')",
        ][self.pick(8)];
        let y = ["0.0", "-0.0", "0.125", "-2.0", "1e308", "float('inf')"][self.pick(6)];
        let op = ["+", "-", "*", "/"][self.pick(4)];
        let mixed = case.is_multiple_of(3);
        let kind = if mixed { "int" } else { "float" };
        let x = if mixed { "9007199254740993" } else { x };
        let n = self.pick(9);
        format!(
            "def f(x: {kind},y: float,data: list[float],n: int):\n    a = x + y\n    b = y\n    total = 0.0\n    for i in range(n):\n        if a < b:\n            a,b = b,a {op} y\n        else:\n            a,b = a {op} y,b\n        for item in data:\n            total += (a {op} item) * b\n    return (a,b,total)\ntry:\n    output = ('return',repr(f({x},{y},[0.0,-0.0,0.5,-1e-308],{n})))\nexcept Exception as error:\n    output = ('raise',type(error).__name__,str(error))\nrepr(output)"
        )
    }
    fn scalar_calls(&mut self, case: u64) -> String {
        let mut expression = ["x", "y", "-x", "~x", "x + y", "x * y"][self.pick(6)].to_owned();
        for _ in 0..self.pick(4) {
            let operand = ["x", "y", "-1", "0", "3"][self.pick(5)];
            let operator = ["+", "-", "*", "//", "%", "&", "|", "^"][self.pick(8)];
            expression = format!("({expression} {operator} {operand})");
        }
        if case.is_multiple_of(3) {
            let operator = ["==", "!=", "<", "<=", ">", ">="][self.pick(6)];
            expression = format!("({expression} {operator} y)");
        }
        let x = [
            "-9223372036854775808",
            "9223372036854775807",
            "9007199254740993",
            "True",
            "0.25",
            "-7",
            "0",
            "13",
        ][self.pick(8)];
        let y = ["-1", "0", "3", "False", "0.5", "9223372036854775808"][self.pick(6)];
        let binding = match case % 3 {
            0 => "f(arg(value), y)",
            1 => "f(x=arg(value), y=y)",
            _ => "f(*(arg(value), y))",
        };
        format!(
            "events = []\ndef arg(value):\n    events.append(value)\n    return value\ndef f(x: int, y: int):\n    return {expression}\ny = {y}\noutput = []\nfor value in [{x},1,-2,0,True,0.5]:\n    try:\n        output.append(('return',repr({binding})))\n    except Exception as error:\n        output.append(('raise',type(error).__name__,str(error)))\nrepr((output,events))"
        )
    }
    fn program(&mut self, case: u64) -> String {
        if case % 16 == 7 {
            let length = 1 + self.pick(64);
            let value = self.small();
            let n = self.pick(9);
            let query = if case.is_multiple_of(23) {
                "None"
            } else if case.is_multiple_of(3) {
                "key"
            } else {
                "(key + 'x')[:-1]"
            };
            return format!(
                "key = 'lookup' * {length}\nrow = {{key: {value}}}\ndef f(row: dict[str,int],key: str,n: int) -> int:\n    total = 0\n    for _ in range(n):\n        total += row[key]\n    return total\ntry:\n    output = ('return',repr(f(row,{query},{n})))\nexcept Exception as error:\n    output = ('raise',type(error).__name__,str(error))\nrepr(output)"
            );
        }
        if case % 8 == 3 {
            return self.scalar_calls(case);
        }
        if case % 4 == 1 {
            return self.floats(case);
        }
        let n = self.pick(9);
        let x = match case % 41 {
            0 => "True".to_owned(),
            1 => "0.5".to_owned(),
            2 => "None".to_owned(),
            3 => "9223372036854775808".to_owned(),
            4 => "-9223372036854775808".to_owned(),
            5 => "9223372036854775807".to_owned(),
            _ => self.small().to_string(),
        };
        let y = if case.is_multiple_of(13) {
            "-1".to_owned()
        } else {
            self.small().to_string()
        };
        let mut data = match case % 37 {
            0 => "[1,2,3]".to_owned(),
            // A string can amplify through generated multiplication before a type
            // error. None exercises the same guard miss without allocating a repeat.
            1 => "[1,2,None,4,5]".to_owned(),
            _ => format!("[{},0,-2,3,{}]", self.small(), self.small()),
        };
        if case.is_multiple_of(101) {
            let length = [0, 1, 4, 5, 6, 4095, 4096, 4097, 10000][self.pick(9)];
            data = format!("list(range({length}))");
        }
        let row = match case % 31 {
            0 => "{}".to_owned(),
            1 => "{'k': 0.25}".to_owned(),
            _ => format!("{{'k': {}}}", self.small()),
        };
        let callback = case.is_multiple_of(47);
        let mut source = String::from("events = []\n");
        if callback {
            source.push_str("class Key:\n    def __hash__(self):\n        return hash('k')\n    def __eq__(self, other):\n        events.append(other)\n        return other == 'k'\n");
        }
        source.push_str(
            "def f(x: int,y: int,data: list[int],row: dict[str,int],n: int):\n    a,b,c = x,y,3\n    total = 0\n",
        );
        if case.is_multiple_of(7) {
            source.push_str("    a,b,c,d,e = data\n");
        }
        let loop_header = match case % 3 {
            0 => "    iterations = range(n)\n    for i in iterations:\n",
            1 => "    for i in range(n):\n",
            _ => "    i = 0\n    while i < n:\n",
        };
        source.push_str(loop_header);
        let vars = ["a", "b", "c", "total", "x", "y", "i"];
        let condition = self.expression(1, &vars);
        let first = self.update(&vars);
        let second = self.update(&vars);
        write!(&mut source,"        if ({condition}) % 3 == 0:\n            a,b,c = b,c,{first}\n        else:\n            a,b,c = {second},a,b\n").unwrap();
        if case.is_multiple_of(2) {
            let expression = self.update(&["a", "b", "c", "total", "item", "i"]);
            write!(
                &mut source,
                "        for item in data:\n            total = {expression}\n"
            )
            .unwrap();
        } else {
            let expression = self.update(&vars);
            writeln!(&mut source, "        total = {expression}").unwrap();
        }
        if case % 3 == 2 {
            source.push_str("        i += 1\n");
        }
        source.push_str("        seen = a\n");
        if case.is_multiple_of(5) {
            source.push_str("    return (a,b,c,total,seen)\n");
        } else {
            source.push_str("    return (a,b,c,total)\n");
        }
        let row = if callback { "{Key(): 7}".to_owned() } else { row };
        write!(&mut source,"try:\n    output = ('return',repr(f({x},{y},{data},{row},{n})))\nexcept Exception as error:\n    output = ('raise',type(error).__name__,str(error))\nrepr((output,events))").unwrap();
        source
    }
}

fn execute(source: &str, backend: Option<Backend>) -> Result<MontyObject, String> {
    experimental_aot::set_enabled(backend.is_some());
    if let Some(backend) = backend {
        experimental_aot::set_backend(backend);
    }
    experimental_aot::reset_stats();
    MontyRun::new(source.to_owned(), "differential.py", vec![], CompileOptions::default())
        .map_err(|error| error.to_string())?
        .run(
            vec![],
            ResourceTracker::new(
                ResourceLimits::default()
                    .max_feed_duration(Duration::from_secs(2))
                    .max_memory(16 * 1024 * 1024),
            ),
            PrintWriter::Disabled,
        )
        .map_err(|error| error.to_string())
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let count = args.get(1).map_or(100_000, |value| value.parse::<u64>().unwrap());
    let seed = args
        .get(2)
        .map_or(0x42_8123_4567_89ab, |value| value.parse::<u64>().unwrap());
    let seconds = args.get(3).map_or(3600, |value| value.parse::<u64>().unwrap());
    let skip = args.get(4).map_or(0, |value| value.parse::<u64>().unwrap());
    let mut random = Random(seed);
    let start = Instant::now();
    let mut returns = [0_u64; 2];
    let mut entries = [0_u64; 2];
    let mut fallbacks = [0_u64; 2];
    let mut fast_calls = [0_u64; 2];
    monty_alloc::set_hard_limit(Some(64 * 1024 * 1024)).unwrap();
    for case in 0..count {
        let source = random.program(case);
        if case < skip {
            continue;
        }
        let expected = execute(&source, None);
        for (index, backend) in [Backend::Copied, Backend::Shared].into_iter().enumerate() {
            let actual = execute(&source, Some(backend));
            if actual != expected || expected.is_err() {
                fs::write("aot-differential-failure.py", &source).unwrap();
                eprintln!(
                    "seed={seed} case={case} backend={backend:?}\nexpected={expected:?}\nactual={actual:?}\n{source}"
                );
                process::exit(1);
            }
            let stats = experimental_aot::stats();
            returns[index] += stats.returns;
            entries[index] += stats.entries;
            fallbacks[index] += stats.fallbacks;
            fast_calls[index] += stats.fast_calls;
        }
        if case.is_multiple_of(1000) || case + 1 == count || start.elapsed().as_secs() >= seconds {
            println!(
                "seed={seed} cases={} seconds={:.3} returns={returns:?} entries={entries:?} fallbacks={fallbacks:?} fast_calls={fast_calls:?}",
                case + 1,
                start.elapsed().as_secs_f64()
            );
        }
        if start.elapsed().as_secs() >= seconds {
            break;
        }
    }
}
