use std::{borrow::Cow, fmt};

use num_bigint::BigInt;

use crate::{
    args::{ArgValues, FromArgs},
    builtins::{Builtins, BuiltinsFunctions, iter::builtin_iter, object_setattr::builtin_object_setattr},
    bytecode::{CallResult, VM},
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunError, RunResult, SimpleException},
    heap::{DropWithContext, Heap, HeapData, HeapId},
    intern::{Interns, StaticStrings, StringId},
    modules::{
        ModuleFunctions, collections, csv,
        hashlib::{self, Blake2Kind},
        itertools,
        itertools::ItertoolsFunctions,
    },
    py_value::PyValue,
    types::{
        Bytes, Deque, Dict, FrozenSet, GenericAlias, List, LongInt, Partial, Path, PyTrait, Random, Range, Set, Slice,
        Str, TimeZone, Tuple,
        bytes::{bytes_fromhex, bytes_repr},
        complex, date, datetime, decimal,
        dict::{DictKind, dict_fromkeys},
        instance::class_name,
        long_int::INT_MAX_STR_DIGITS,
        path,
        str::{StringRepr, allocate_string},
        time,
        timedelta::{self, DAY_MICROSECONDS, MAX_TIMEDELTA_DAYS, MIN_TIMEDELTA_DAYS},
        timezone::{self, MAX_TIMEZONE_CONSTANT_SECONDS},
    },
    value::{EitherStr, Value},
};

/// Python types used for runtime dispatch, including borrowed class identities.
/// Variant names are persisted in dumps; renames require a serde alias.
/// Variant order is also used by builtin id() through postcard.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantNames,
)]
#[strum(serialize_all = "lowercase")]
#[expect(
    clippy::enum_variant_names,
    reason = "`Type` and `NoneType` mirror the Python type names"
)]
pub enum Type {
    Ellipsis,
    #[strum(serialize = "NotImplementedType")]
    NotImplementedType,
    Type,
    #[strum(serialize = "NoneType")]
    NoneType,
    Bool,
    Int,
    Float,
    Complex,
    Range,
    Slice,
    /// The four `datetime` classes are qualified like `collections.deque`:
    /// this is the `tp_name` CPython gives these C types, so it is the
    /// spelling its reprs and type-naming error messages use. `__name__`
    /// reports the bare name, see `dunder_name`.
    #[strum(serialize = "datetime.date")]
    Date,
    #[strum(serialize = "datetime.datetime")]
    DateTime,
    #[strum(serialize = "datetime.timedelta")]
    TimeDelta,
    #[strum(serialize = "datetime.timezone")]
    TimeZone,
    Str,
    Bytes,
    List,
    Tuple,
    NamedTuple,
    Dict,
    /// `collections.defaultdict` — a `dict` with a `default_factory` (stored as
    /// a `DictKind` on the `Dict`, not a separate heap type).
    #[strum(serialize = "collections.defaultdict")]
    DefaultDict,
    /// `collections.Counter` — a `dict` subclass (also a `DictKind`).
    #[strum(serialize = "Counter")]
    Counter,
    #[strum(serialize = "dict_keys")]
    DictKeys,
    #[strum(serialize = "dict_items")]
    DictItems,
    #[strum(serialize = "dict_values")]
    DictValues,
    Set,
    FrozenSet,
    /// The type of a host-backed class instance ([`HeapData::HostClass`]),
    /// for internal dispatch only: `type(x)` materializes a `HostClassType`
    /// instead, so it never reaches Python code or the host boundary.
    ///
    /// [`HeapData::HostClass`]: crate::heap::HeapData::HostClass
    #[strum(serialize = "HostClass")]
    HostClass,
    /// A user-defined instance's class, borrowed for runtime type checks and names.
    /// The class object itself reports `Type::Type`.
    ///
    /// The id is non-owning: this `Copy` type does not participate in refcounting.
    /// It is valid only while the source value keeps its class alive. Never retain it
    /// in snapshots or constant pools; type objects use `BuiltinType` or a heap class.
    #[strum(disabled)]
    Instance(HeapId),
    /// Exception types render/parse via `ExcType`'s own strum name
    /// (`"ValueError"`, `"json.JSONDecodeError"`, ...), so this variant is
    /// `#[strum(disabled)]`: every strum consumer (`Display`, [`Type::name`],
    /// [`Type::from_type_name`]) peels `Exception` off explicitly, and
    /// enabling it would make `EnumString` accept the meaningless
    /// `"exception"`.
    #[strum(disabled)]
    Exception(ExcType),
    Function,
    #[strum(serialize = "builtin_function_or_method")]
    BuiltinFunction,
    Cell,
    Iterator,
    #[strum(serialize = "list_iterator")]
    ListIterator,
    #[strum(serialize = "callable_iterator")]
    CallableIterator,
    /// Coroutine type for async functions and external futures.
    Coroutine,
    Module,
    /// Marker types like stdout/stderr - displays as "_io.TextIOWrapper"
    #[strum(serialize = "_io.TextIOWrapper")]
    TextIOWrapper,
    /// Binary file object returned by `open(..., "rb")`.
    #[strum(serialize = "_io.BufferedReader")]
    BufferedReader,
    /// Binary file object returned by write-only binary modes.
    #[strum(serialize = "_io.BufferedWriter")]
    BufferedWriter,
    /// Binary file object returned by read/write binary modes.
    #[strum(serialize = "_io.BufferedRandom")]
    BufferedRandom,
    /// typing module special forms (Any, Optional, Union, etc.) - displays as "typing._SpecialForm"
    #[strum(serialize = "typing._SpecialForm")]
    SpecialForm,
    /// A filesystem path from `pathlib.Path` - displays as "PosixPath"
    #[strum(serialize = "PosixPath")]
    Path,
    /// A property descriptor - displays as "property"
    Property,
    /// A compiled regex pattern from `re.compile()` - displays as "re.Pattern"
    #[strum(serialize = "re.Pattern")]
    RePattern,
    /// A regex match result from `re.match()` / `re.search()` etc. - displays as "re.Match"
    #[strum(serialize = "re.Match")]
    ReMatch,
    /// An `os.DirEntry` from `os.scandir()`.
    #[strum(serialize = "posix.DirEntry")]
    DirEntry,
    /// The iterator `os.scandir()` returns.
    #[strum(serialize = "posix.ScandirIterator")]
    ScandirIterator,
    /// The iterator `os.walk()` and `Path.walk()` return; CPython's is a generator.
    #[strum(serialize = "generator")]
    WalkGenerator,
    // Variants serialize by name into dumps: renaming one needs `#[serde(alias)]`.
    #[strum(serialize = "tuple_iterator")]
    TupleIterator,
    #[strum(serialize = "str_ascii_iterator")]
    StrAsciiIterator,
    #[strum(serialize = "str_iterator")]
    StrIterator,
    #[strum(serialize = "bytes_iterator")]
    BytesIterator,
    #[strum(serialize = "range_iterator")]
    RangeIterator,
    #[strum(serialize = "dict_keyiterator")]
    DictKeyIterator,
    #[strum(serialize = "dict_itemiterator")]
    DictItemIterator,
    #[strum(serialize = "dict_valueiterator")]
    DictValueIterator,
    #[strum(serialize = "set_iterator")]
    SetIterator,
    #[strum(serialize = "itertools.count")]
    ItertoolsCount,
    #[strum(serialize = "itertools.repeat")]
    ItertoolsRepeat,
    /// A `dataclasses.Field` from a class's `__dataclass_fields__` — displays
    /// as "Field", the name CPython's `Field.__name__` reports.
    #[strum(serialize = "Field")]
    DataclassField,
    /// `collections.deque` — qualified like `datetime.datetime`/`re.Pattern` so
    /// the name matches CPython's `repr` and error messages; `__name__` reports
    /// the bare `'deque'`, see `dunder_name`.
    #[strum(serialize = "collections.deque")]
    Deque,
    /// `iter(deque(...))` — CPython's `_collections._deque_iterator`.
    #[strum(serialize = "_collections._deque_iterator")]
    DequeIterator,
    #[strum(serialize = "itertools.pairwise")]
    ItertoolsPairwise,
    #[strum(serialize = "itertools.compress")]
    ItertoolsCompress,
    #[strum(serialize = "itertools.islice")]
    ItertoolsIslice,
    #[strum(serialize = "itertools.chain")]
    ItertoolsChain,
    #[strum(serialize = "itertools.cycle")]
    ItertoolsCycle,
    /// The `__dataclass_params__` of a `@dataclass`, named as CPython's
    /// private `dataclasses._DataclassParams` reports itself.
    #[strum(serialize = "_DataclassParams")]
    DataclassParams,
    /// The type of `dataclasses.MISSING`, CPython's private `_MISSING_TYPE`.
    #[strum(serialize = "dataclasses._MISSING_TYPE")]
    MissingType,
    #[strum(serialize = "itertools.takewhile")]
    ItertoolsTakeWhile,
    #[strum(serialize = "itertools.dropwhile")]
    ItertoolsDropWhile,
    #[strum(serialize = "itertools.filterfalse")]
    ItertoolsFilterFalse,
    #[strum(serialize = "itertools.starmap")]
    ItertoolsStarMap,
    /// `object` — the builtin name only, not a base class: Monty has no
    /// inheritance, so it exists to carry `object.__setattr__`, the write that
    /// bypasses a class's attribute hooks. Constructing it is unsupported.
    Object,
    #[strum(serialize = "datetime.time")]
    Time,
    /// `functools.partial` — qualified like `collections.deque`, so
    /// `type(p)` reads `<class 'functools.partial'>`.
    #[strum(serialize = "functools.partial")]
    Partial,
    #[strum(serialize = "itertools.accumulate")]
    ItertoolsAccumulate,
    #[strum(serialize = "itertools.batched")]
    ItertoolsBatched,
    #[strum(serialize = "itertools.zip_longest")]
    ItertoolsZipLongest,
    /// `types.GenericAlias`, the type of `list[int]`; qualified like
    /// `functools.partial`, so `type(list[int])` reads `<class 'types.GenericAlias'>`.
    #[strum(serialize = "types.GenericAlias")]
    GenericAlias,
    /// `typing.Union`, the type of `int | None` — one object with
    /// `types.UnionType` since CPython 3.14, and the value bound to
    /// `typing.Union` itself.
    #[strum(serialize = "typing.Union")]
    Union,
    #[strum(serialize = "itertools.combinations")]
    ItertoolsCombinations,
    #[strum(serialize = "itertools.combinations_with_replacement")]
    ItertoolsCombinationsWithReplacement,
    #[strum(serialize = "itertools.permutations")]
    ItertoolsPermutations,
    #[strum(serialize = "itertools.product")]
    ItertoolsProduct,
    #[strum(serialize = "itertools.groupby")]
    ItertoolsGroupBy,
    /// The sub-iterator `groupby` yields for each group — CPython's private
    /// `itertools._grouper`, named as such so `type()` matches.
    #[strum(serialize = "itertools._grouper")]
    ItertoolsGrouper,
    /// The iterator `tee()` hands out — CPython's private `itertools._tee`,
    /// which it nonetheless exposes on the module.
    #[strum(serialize = "itertools._tee")]
    ItertoolsTee,
    /// The read-ahead buffer those iterators share, CPython's
    /// `itertools._tee_dataobject`. Never reachable from Python except as
    /// that name.
    #[strum(serialize = "itertools._tee_dataobject")]
    ItertoolsTeeDataObject,
    /// `random.Random`, qualified like `collections.deque` so `type(rng)`
    /// reads `<class 'random.Random'>`.
    #[strum(serialize = "random.Random")]
    Random,
    /// The wrapper returned by caching decorators. Monty exposes this type
    /// through `type(f)`, but not as a `functools` module attribute.
    #[strum(serialize = "functools._lru_cache_wrapper")]
    LruCacheWrapper,
    /// The OpenSSL-backed `hashlib` hash object, `_hashlib.HASH`.
    #[strum(serialize = "_hashlib.HASH")]
    HashlibHash,
    /// The `shake_128` / `shake_256` hash object, `_hashlib.HASHXOF`.
    #[strum(serialize = "_hashlib.HASHXOF")]
    HashlibHashXof,
    /// `hashlib.blake2b`, a type in CPython and so callable here.
    #[strum(serialize = "_blake2.blake2b")]
    Blake2b,
    /// `hashlib.blake2s`, a type in CPython and so callable here.
    #[strum(serialize = "_blake2.blake2s")]
    Blake2s,
    /// The iterator `csv.reader()` returns, CPython's `_csv.reader`.
    #[strum(serialize = "_csv.reader")]
    CsvReader,
    /// The object `csv.writer()` returns, CPython's `_csv.writer`.
    #[strum(serialize = "_csv.writer")]
    CsvWriter,
    /// `csv.DictReader`, a class in CPython and so callable here.
    #[strum(serialize = "csv.DictReader")]
    CsvDictReader,
    /// `csv.DictWriter`, a class in CPython and so callable here.
    #[strum(serialize = "csv.DictWriter")]
    CsvDictWriter,
    /// A resolved `csv` dialect, CPython's `_csv.Dialect`.
    #[strum(serialize = "_csv.Dialect")]
    CsvDialect,
    /// `decimal.Decimal`.
    #[strum(serialize = "decimal.Decimal")]
    Decimal,
    /// `decimal.Context`.
    #[strum(serialize = "decimal.Context")]
    DecimalContext,
    /// The type of a context's `flags` / `traps`; CPython defines it in Python, so
    /// its name carries no module.
    #[strum(serialize = "SignalDict")]
    DecimalSignalDict,
    /// The manager `decimal.localcontext()` returns.
    #[strum(serialize = "decimal.ContextManager")]
    DecimalContextManager,
}

/// Type objects stored inline in values; user classes use heap references.
/// Serialize through `Type` to retain its dump names and postcard indices,
/// including the gap left by `Type::Instance`. Keep both conversions exhaustive.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantNames,
)]
#[strum(serialize_all = "lowercase")]
#[serde(into = "Type", try_from = "Type")]
pub(crate) enum BuiltinType {
    Ellipsis,
    #[strum(serialize = "NotImplementedType")]
    NotImplementedType,
    Type,
    #[strum(serialize = "NoneType")]
    NoneType,
    Bool,
    Int,
    Float,
    Complex,
    Range,
    Slice,
    #[strum(serialize = "datetime.date")]
    Date,
    #[strum(serialize = "datetime.datetime")]
    DateTime,
    #[strum(serialize = "datetime.timedelta")]
    TimeDelta,
    #[strum(serialize = "datetime.timezone")]
    TimeZone,
    Str,
    Bytes,
    List,
    Tuple,
    NamedTuple,
    Dict,
    #[strum(serialize = "collections.defaultdict")]
    DefaultDict,
    #[strum(serialize = "Counter")]
    Counter,
    #[strum(serialize = "dict_keys")]
    DictKeys,
    #[strum(serialize = "dict_items")]
    DictItems,
    #[strum(serialize = "dict_values")]
    DictValues,
    Set,
    FrozenSet,
    #[strum(serialize = "HostClass")]
    HostClass,
    #[strum(disabled)]
    Exception(ExcType),
    Function,
    #[strum(serialize = "builtin_function_or_method")]
    BuiltinFunction,
    Cell,
    Iterator,
    #[strum(serialize = "list_iterator")]
    ListIterator,
    #[strum(serialize = "callable_iterator")]
    CallableIterator,
    Coroutine,
    Module,
    #[strum(serialize = "_io.TextIOWrapper")]
    TextIOWrapper,
    #[strum(serialize = "_io.BufferedReader")]
    BufferedReader,
    #[strum(serialize = "_io.BufferedWriter")]
    BufferedWriter,
    #[strum(serialize = "_io.BufferedRandom")]
    BufferedRandom,
    #[strum(serialize = "typing._SpecialForm")]
    SpecialForm,
    #[strum(serialize = "PosixPath")]
    Path,
    Property,
    #[strum(serialize = "re.Pattern")]
    RePattern,
    #[strum(serialize = "re.Match")]
    ReMatch,
    #[strum(serialize = "posix.DirEntry")]
    DirEntry,
    #[strum(serialize = "posix.ScandirIterator")]
    ScandirIterator,
    #[strum(serialize = "generator")]
    WalkGenerator,
    #[strum(serialize = "tuple_iterator")]
    TupleIterator,
    #[strum(serialize = "str_ascii_iterator")]
    StrAsciiIterator,
    #[strum(serialize = "str_iterator")]
    StrIterator,
    #[strum(serialize = "bytes_iterator")]
    BytesIterator,
    #[strum(serialize = "range_iterator")]
    RangeIterator,
    #[strum(serialize = "dict_keyiterator")]
    DictKeyIterator,
    #[strum(serialize = "dict_itemiterator")]
    DictItemIterator,
    #[strum(serialize = "dict_valueiterator")]
    DictValueIterator,
    #[strum(serialize = "set_iterator")]
    SetIterator,
    #[strum(serialize = "itertools.count")]
    ItertoolsCount,
    #[strum(serialize = "itertools.repeat")]
    ItertoolsRepeat,
    #[strum(serialize = "Field")]
    DataclassField,
    #[strum(serialize = "collections.deque")]
    Deque,
    #[strum(serialize = "_collections._deque_iterator")]
    DequeIterator,
    #[strum(serialize = "itertools.pairwise")]
    ItertoolsPairwise,
    #[strum(serialize = "itertools.compress")]
    ItertoolsCompress,
    #[strum(serialize = "itertools.islice")]
    ItertoolsIslice,
    #[strum(serialize = "itertools.chain")]
    ItertoolsChain,
    #[strum(serialize = "itertools.cycle")]
    ItertoolsCycle,
    #[strum(serialize = "_DataclassParams")]
    DataclassParams,
    #[strum(serialize = "dataclasses._MISSING_TYPE")]
    MissingType,
    #[strum(serialize = "itertools.takewhile")]
    ItertoolsTakeWhile,
    #[strum(serialize = "itertools.dropwhile")]
    ItertoolsDropWhile,
    #[strum(serialize = "itertools.filterfalse")]
    ItertoolsFilterFalse,
    #[strum(serialize = "itertools.starmap")]
    ItertoolsStarMap,
    Object,
    #[strum(serialize = "datetime.time")]
    Time,
    #[strum(serialize = "functools.partial")]
    Partial,
    #[strum(serialize = "itertools.accumulate")]
    ItertoolsAccumulate,
    #[strum(serialize = "itertools.batched")]
    ItertoolsBatched,
    #[strum(serialize = "itertools.zip_longest")]
    ItertoolsZipLongest,
    #[strum(serialize = "types.GenericAlias")]
    GenericAlias,
    #[strum(serialize = "typing.Union")]
    Union,
    #[strum(serialize = "itertools.combinations")]
    ItertoolsCombinations,
    #[strum(serialize = "itertools.combinations_with_replacement")]
    ItertoolsCombinationsWithReplacement,
    #[strum(serialize = "itertools.permutations")]
    ItertoolsPermutations,
    #[strum(serialize = "itertools.product")]
    ItertoolsProduct,
    #[strum(serialize = "itertools.groupby")]
    ItertoolsGroupBy,
    #[strum(serialize = "itertools._grouper")]
    ItertoolsGrouper,
    #[strum(serialize = "itertools._tee")]
    ItertoolsTee,
    #[strum(serialize = "itertools._tee_dataobject")]
    ItertoolsTeeDataObject,
    #[strum(serialize = "random.Random")]
    Random,
    #[strum(serialize = "functools._lru_cache_wrapper")]
    LruCacheWrapper,
    #[strum(serialize = "_hashlib.HASH")]
    HashlibHash,
    #[strum(serialize = "_hashlib.HASHXOF")]
    HashlibHashXof,
    #[strum(serialize = "_blake2.blake2b")]
    Blake2b,
    #[strum(serialize = "_blake2.blake2s")]
    Blake2s,
    #[strum(serialize = "_csv.reader")]
    CsvReader,
    #[strum(serialize = "_csv.writer")]
    CsvWriter,
    #[strum(serialize = "csv.DictReader")]
    CsvDictReader,
    #[strum(serialize = "csv.DictWriter")]
    CsvDictWriter,
    #[strum(serialize = "_csv.Dialect")]
    CsvDialect,
}

impl From<BuiltinType> for Type {
    fn from(value: BuiltinType) -> Self {
        match value {
            BuiltinType::Ellipsis => Self::Ellipsis,
            BuiltinType::NotImplementedType => Self::NotImplementedType,
            BuiltinType::Type => Self::Type,
            BuiltinType::NoneType => Self::NoneType,
            BuiltinType::Bool => Self::Bool,
            BuiltinType::Int => Self::Int,
            BuiltinType::Float => Self::Float,
            BuiltinType::Complex => Self::Complex,
            BuiltinType::Range => Self::Range,
            BuiltinType::Slice => Self::Slice,
            BuiltinType::Date => Self::Date,
            BuiltinType::DateTime => Self::DateTime,
            BuiltinType::TimeDelta => Self::TimeDelta,
            BuiltinType::TimeZone => Self::TimeZone,
            BuiltinType::Str => Self::Str,
            BuiltinType::Bytes => Self::Bytes,
            BuiltinType::List => Self::List,
            BuiltinType::Tuple => Self::Tuple,
            BuiltinType::NamedTuple => Self::NamedTuple,
            BuiltinType::Dict => Self::Dict,
            BuiltinType::DefaultDict => Self::DefaultDict,
            BuiltinType::Counter => Self::Counter,
            BuiltinType::DictKeys => Self::DictKeys,
            BuiltinType::DictItems => Self::DictItems,
            BuiltinType::DictValues => Self::DictValues,
            BuiltinType::Set => Self::Set,
            BuiltinType::FrozenSet => Self::FrozenSet,
            BuiltinType::HostClass => Self::HostClass,
            BuiltinType::Exception(exc_type) => Self::Exception(exc_type),
            BuiltinType::Function => Self::Function,
            BuiltinType::BuiltinFunction => Self::BuiltinFunction,
            BuiltinType::Cell => Self::Cell,
            BuiltinType::Iterator => Self::Iterator,
            BuiltinType::ListIterator => Self::ListIterator,
            BuiltinType::CallableIterator => Self::CallableIterator,
            BuiltinType::Coroutine => Self::Coroutine,
            BuiltinType::Module => Self::Module,
            BuiltinType::TextIOWrapper => Self::TextIOWrapper,
            BuiltinType::BufferedReader => Self::BufferedReader,
            BuiltinType::BufferedWriter => Self::BufferedWriter,
            BuiltinType::BufferedRandom => Self::BufferedRandom,
            BuiltinType::SpecialForm => Self::SpecialForm,
            BuiltinType::Path => Self::Path,
            BuiltinType::Property => Self::Property,
            BuiltinType::RePattern => Self::RePattern,
            BuiltinType::ReMatch => Self::ReMatch,
            BuiltinType::DirEntry => Self::DirEntry,
            BuiltinType::ScandirIterator => Self::ScandirIterator,
            BuiltinType::WalkGenerator => Self::WalkGenerator,
            BuiltinType::TupleIterator => Self::TupleIterator,
            BuiltinType::StrAsciiIterator => Self::StrAsciiIterator,
            BuiltinType::StrIterator => Self::StrIterator,
            BuiltinType::BytesIterator => Self::BytesIterator,
            BuiltinType::RangeIterator => Self::RangeIterator,
            BuiltinType::DictKeyIterator => Self::DictKeyIterator,
            BuiltinType::DictItemIterator => Self::DictItemIterator,
            BuiltinType::DictValueIterator => Self::DictValueIterator,
            BuiltinType::SetIterator => Self::SetIterator,
            BuiltinType::ItertoolsCount => Self::ItertoolsCount,
            BuiltinType::ItertoolsRepeat => Self::ItertoolsRepeat,
            BuiltinType::DataclassField => Self::DataclassField,
            BuiltinType::Deque => Self::Deque,
            BuiltinType::DequeIterator => Self::DequeIterator,
            BuiltinType::ItertoolsPairwise => Self::ItertoolsPairwise,
            BuiltinType::ItertoolsCompress => Self::ItertoolsCompress,
            BuiltinType::ItertoolsIslice => Self::ItertoolsIslice,
            BuiltinType::ItertoolsChain => Self::ItertoolsChain,
            BuiltinType::ItertoolsCycle => Self::ItertoolsCycle,
            BuiltinType::DataclassParams => Self::DataclassParams,
            BuiltinType::MissingType => Self::MissingType,
            BuiltinType::ItertoolsTakeWhile => Self::ItertoolsTakeWhile,
            BuiltinType::ItertoolsDropWhile => Self::ItertoolsDropWhile,
            BuiltinType::ItertoolsFilterFalse => Self::ItertoolsFilterFalse,
            BuiltinType::ItertoolsStarMap => Self::ItertoolsStarMap,
            BuiltinType::Object => Self::Object,
            BuiltinType::Time => Self::Time,
            BuiltinType::Partial => Self::Partial,
            BuiltinType::ItertoolsAccumulate => Self::ItertoolsAccumulate,
            BuiltinType::ItertoolsBatched => Self::ItertoolsBatched,
            BuiltinType::ItertoolsZipLongest => Self::ItertoolsZipLongest,
            BuiltinType::GenericAlias => Self::GenericAlias,
            BuiltinType::Union => Self::Union,
            BuiltinType::ItertoolsCombinations => Self::ItertoolsCombinations,
            BuiltinType::ItertoolsCombinationsWithReplacement => Self::ItertoolsCombinationsWithReplacement,
            BuiltinType::ItertoolsPermutations => Self::ItertoolsPermutations,
            BuiltinType::ItertoolsProduct => Self::ItertoolsProduct,
            BuiltinType::ItertoolsGroupBy => Self::ItertoolsGroupBy,
            BuiltinType::ItertoolsGrouper => Self::ItertoolsGrouper,
            BuiltinType::ItertoolsTee => Self::ItertoolsTee,
            BuiltinType::ItertoolsTeeDataObject => Self::ItertoolsTeeDataObject,
            BuiltinType::Random => Self::Random,
            BuiltinType::LruCacheWrapper => Self::LruCacheWrapper,
            BuiltinType::HashlibHash => Self::HashlibHash,
            BuiltinType::HashlibHashXof => Self::HashlibHashXof,
            BuiltinType::Blake2b => Self::Blake2b,
            BuiltinType::Blake2s => Self::Blake2s,
            BuiltinType::CsvReader => Self::CsvReader,
            BuiltinType::CsvWriter => Self::CsvWriter,
            BuiltinType::CsvDictReader => Self::CsvDictReader,
            BuiltinType::CsvDictWriter => Self::CsvDictWriter,
            BuiltinType::CsvDialect => Self::CsvDialect,
        }
    }
}

impl TryFrom<Type> for BuiltinType {
    type Error = &'static str;

    fn try_from(value: Type) -> Result<Self, Self::Error> {
        Ok(match value {
            Type::Ellipsis => Self::Ellipsis,
            Type::NotImplementedType => Self::NotImplementedType,
            Type::Type => Self::Type,
            Type::NoneType => Self::NoneType,
            Type::Bool => Self::Bool,
            Type::Int => Self::Int,
            Type::Float => Self::Float,
            Type::Complex => Self::Complex,
            Type::Range => Self::Range,
            Type::Slice => Self::Slice,
            Type::Date => Self::Date,
            Type::DateTime => Self::DateTime,
            Type::TimeDelta => Self::TimeDelta,
            Type::TimeZone => Self::TimeZone,
            Type::Str => Self::Str,
            Type::Bytes => Self::Bytes,
            Type::List => Self::List,
            Type::Tuple => Self::Tuple,
            Type::NamedTuple => Self::NamedTuple,
            Type::Dict => Self::Dict,
            Type::DefaultDict => Self::DefaultDict,
            Type::Counter => Self::Counter,
            Type::DictKeys => Self::DictKeys,
            Type::DictItems => Self::DictItems,
            Type::DictValues => Self::DictValues,
            Type::Set => Self::Set,
            Type::FrozenSet => Self::FrozenSet,
            Type::HostClass => Self::HostClass,
            Type::Exception(exc_type) => Self::Exception(exc_type),
            Type::Function => Self::Function,
            Type::BuiltinFunction => Self::BuiltinFunction,
            Type::Cell => Self::Cell,
            Type::Iterator => Self::Iterator,
            Type::ListIterator => Self::ListIterator,
            Type::CallableIterator => Self::CallableIterator,
            Type::Coroutine => Self::Coroutine,
            Type::Module => Self::Module,
            Type::TextIOWrapper => Self::TextIOWrapper,
            Type::BufferedReader => Self::BufferedReader,
            Type::BufferedWriter => Self::BufferedWriter,
            Type::BufferedRandom => Self::BufferedRandom,
            Type::SpecialForm => Self::SpecialForm,
            Type::Path => Self::Path,
            Type::Property => Self::Property,
            Type::RePattern => Self::RePattern,
            Type::ReMatch => Self::ReMatch,
            Type::DirEntry => Self::DirEntry,
            Type::ScandirIterator => Self::ScandirIterator,
            Type::WalkGenerator => Self::WalkGenerator,
            Type::TupleIterator => Self::TupleIterator,
            Type::StrAsciiIterator => Self::StrAsciiIterator,
            Type::StrIterator => Self::StrIterator,
            Type::BytesIterator => Self::BytesIterator,
            Type::RangeIterator => Self::RangeIterator,
            Type::DictKeyIterator => Self::DictKeyIterator,
            Type::DictItemIterator => Self::DictItemIterator,
            Type::DictValueIterator => Self::DictValueIterator,
            Type::SetIterator => Self::SetIterator,
            Type::ItertoolsCount => Self::ItertoolsCount,
            Type::ItertoolsRepeat => Self::ItertoolsRepeat,
            Type::DataclassField => Self::DataclassField,
            Type::Deque => Self::Deque,
            Type::DequeIterator => Self::DequeIterator,
            Type::ItertoolsPairwise => Self::ItertoolsPairwise,
            Type::ItertoolsCompress => Self::ItertoolsCompress,
            Type::ItertoolsIslice => Self::ItertoolsIslice,
            Type::ItertoolsChain => Self::ItertoolsChain,
            Type::ItertoolsCycle => Self::ItertoolsCycle,
            Type::DataclassParams => Self::DataclassParams,
            Type::MissingType => Self::MissingType,
            Type::ItertoolsTakeWhile => Self::ItertoolsTakeWhile,
            Type::ItertoolsDropWhile => Self::ItertoolsDropWhile,
            Type::ItertoolsFilterFalse => Self::ItertoolsFilterFalse,
            Type::ItertoolsStarMap => Self::ItertoolsStarMap,
            Type::Object => Self::Object,
            Type::Time => Self::Time,
            Type::Partial => Self::Partial,
            Type::ItertoolsAccumulate => Self::ItertoolsAccumulate,
            Type::ItertoolsBatched => Self::ItertoolsBatched,
            Type::ItertoolsZipLongest => Self::ItertoolsZipLongest,
            Type::GenericAlias => Self::GenericAlias,
            Type::Union => Self::Union,
            Type::ItertoolsCombinations => Self::ItertoolsCombinations,
            Type::ItertoolsCombinationsWithReplacement => Self::ItertoolsCombinationsWithReplacement,
            Type::ItertoolsPermutations => Self::ItertoolsPermutations,
            Type::ItertoolsProduct => Self::ItertoolsProduct,
            Type::ItertoolsGroupBy => Self::ItertoolsGroupBy,
            Type::ItertoolsGrouper => Self::ItertoolsGrouper,
            Type::ItertoolsTee => Self::ItertoolsTee,
            Type::ItertoolsTeeDataObject => Self::ItertoolsTeeDataObject,
            Type::Random => Self::Random,
            Type::LruCacheWrapper => Self::LruCacheWrapper,
            Type::HashlibHash => Self::HashlibHash,
            Type::HashlibHashXof => Self::HashlibHashXof,
            Type::Blake2b => Self::Blake2b,
            Type::Blake2s => Self::Blake2s,
            Type::CsvReader => Self::CsvReader,
            Type::CsvWriter => Self::CsvWriter,
            Type::CsvDictReader => Self::CsvDictReader,
            Type::CsvDictWriter => Self::CsvDictWriter,
            Type::CsvDialect => Self::CsvDialect,
            Type::Instance(_) => return Err("user-defined classes use heap type objects"),
        })
    }
}

impl fmt::Display for BuiltinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Type::from(*self).fmt(f)
    }
}

/// Writes static type names, delegating exception names to `ExcType`.
///
/// # Panics
/// `Instance` requires heap access to resolve its class name; use [`Type::name`].
impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Exception(exc_type) => exc_type.into(),
            Self::Instance(_) => unreachable!("Type::Instance must be rendered via Type::name"),
            other => other.into(),
        })
    }
}

impl Type {
    /// The Python-visible name of this type: the real class name for
    /// [`Instance`](Self::Instance), the static `Display` name otherwise —
    /// the primary way to render a `Type` in error messages and reprs. The
    /// result borrows only `interns` (never the heap — heap-owned dynamic
    /// class names are cloned into `Cow::Owned`), so it can be captured
    /// before heap-mutating cleanup (`drop_with`) at error sites and
    /// formatted after.
    pub(crate) fn name<'i>(self, heap: &Heap, interns: &'i Interns) -> Cow<'i, str> {
        match self {
            Self::Instance(class_id) => class_name(class_id, heap, interns),
            Self::Exception(exc_type) => Cow::Borrowed(exc_type.into()),
            other => Cow::Borrowed(other.into()),
        }
    }

    /// The name CPython's `__name__` reports: [`name`](Self::name) with any
    /// module qualifier stripped (`datetime.date` → `date`). CPython keeps one
    /// dotted `tp_name` per C type and derives the bare `__name__` from it, so
    /// reprs and error messages qualify where `__name__` does not. Sandbox
    /// class names ([`Instance`](Self::Instance)) are identifiers, so
    /// stripping is a no-op for them.
    pub(crate) fn dunder_name<'i>(self, heap: &Heap, interns: &'i Interns) -> Cow<'i, str> {
        match self.name(heap, interns) {
            Cow::Borrowed(name) => Cow::Borrowed(name.rsplit_once('.').map_or(name, |(_, bare)| bare)),
            owned @ Cow::Owned(_) => owned,
        }
    }

    /// [`name`](Self::name) as rendered by CPython's `_PyArg_BadArgument`
    /// ("argument N must be X, not Y") error formatter: identical except that
    /// `NoneType` renders as `"None"` — CPython special-cases
    /// `arg == Py_None ? "None" : Py_TYPE(arg)->tp_name`, and since `NoneType`
    /// is a singleton, branching on the type is equivalent to branching on the
    /// value. Use for the "not Y" half of arg-type error messages only.
    pub(crate) fn cpython_arg_name<'i>(self, heap: &Heap, interns: &'i Interns) -> Cow<'i, str> {
        match self {
            Self::NoneType => Cow::Borrowed("None"),
            other => other.name(heap, interns),
        }
    }

    /// Returns the Python source-level name for builtin types that can be called directly.
    ///
    /// This differs from `Display` for internal representation-only names such as
    /// `Type::Iterator`, which displays as `iterator` for repr/type output but is
    /// exposed as the builtin constructor `iter` in Python source.
    #[must_use]
    pub const fn builtin_name(self) -> Option<&'static str> {
        match self {
            Self::Bool => Some("bool"),
            Self::Int => Some("int"),
            Self::Float => Some("float"),
            Self::Complex => Some("complex"),
            Self::Str => Some("str"),
            Self::Bytes => Some("bytes"),
            Self::List => Some("list"),
            Self::Tuple => Some("tuple"),
            Self::Dict => Some("dict"),
            Self::Set => Some("set"),
            Self::FrozenSet => Some("frozenset"),
            Self::Range => Some("range"),
            Self::Slice => Some("slice"),
            Self::Iterator => Some("iter"),
            Self::Type => Some("type"),
            Self::Property => Some("property"),
            _ => None,
        }
    }

    /// Resolves a bare Python name to a builtin type, if it is one.
    ///
    /// Only matches names that are true Python builtins — accessible without any import.
    /// Internal types like `TextIOWrapper`, `PosixPath`, `NoneType`, and `ellipsis` are
    /// intentionally excluded because they require imports or are not directly nameable.
    ///
    /// This replaces the previous strum `FromStr` derive which matched ALL variants,
    /// including internal types that shouldn't be resolvable from bare names.
    #[must_use]
    pub fn from_builtin_name(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "complex" => Some(Self::Complex),
            "str" => Some(Self::Str),
            "bytes" => Some(Self::Bytes),
            "list" => Some(Self::List),
            "tuple" => Some(Self::Tuple),
            "dict" => Some(Self::Dict),
            "set" => Some(Self::Set),
            "frozenset" => Some(Self::FrozenSet),
            "range" => Some(Self::Range),
            "slice" => Some(Self::Slice),
            "iter" => Some(Self::Iterator),
            "type" => Some(Self::Type),
            "property" => Some(Self::Property),
            "object" => Some(Self::Object),
            _ => None,
        }
    }

    /// Whether subscripting this type builds a `types.GenericAlias`
    /// (`list[int]`), i.e. whether CPython's type defines `__class_getitem__`.
    ///
    /// `type` is included even though Monty binds the name to a builtin
    /// function: `type[int]` is routed here by `Value::py_getitem`.
    #[must_use]
    pub(crate) const fn has_class_getitem(self) -> bool {
        matches!(
            self,
            Self::List
                | Self::Tuple
                | Self::Dict
                | Self::DefaultDict
                | Self::Counter
                | Self::Set
                | Self::FrozenSet
                | Self::Type
                | Self::Deque
                | Self::Partial
                | Self::RePattern
                | Self::ReMatch
                // The one `itertools` type CPython gives a
                // `__class_getitem__`; the rest reject a subscript.
                | Self::ItertoolsChain
        )
    }

    /// Whether multiplication can fall back to CPython's sequence-repetition protocol.
    #[must_use]
    pub(crate) const fn has_sequence_repeat(self) -> bool {
        matches!(
            self,
            Self::Str | Self::Bytes | Self::List | Self::Tuple | Self::NamedTuple | Self::Deque
        )
    }

    /// Returns whether this is one of Python's concrete iterator types.
    #[must_use]
    pub(crate) const fn is_iterator(self) -> bool {
        matches!(
            self,
            Self::ListIterator
                | Self::DequeIterator
                | Self::TupleIterator
                | Self::StrAsciiIterator
                | Self::StrIterator
                | Self::BytesIterator
                | Self::RangeIterator
                | Self::DictKeyIterator
                | Self::DictItemIterator
                | Self::DictValueIterator
                | Self::SetIterator
                | Self::CallableIterator
                | Self::ScandirIterator
                | Self::WalkGenerator
                | Self::ItertoolsCount
                | Self::ItertoolsRepeat
                | Self::ItertoolsPairwise
                | Self::ItertoolsCompress
                | Self::ItertoolsIslice
                | Self::ItertoolsChain
                | Self::ItertoolsCycle
                | Self::ItertoolsTakeWhile
                | Self::ItertoolsDropWhile
                | Self::ItertoolsFilterFalse
                | Self::ItertoolsStarMap
                | Self::ItertoolsAccumulate
                | Self::ItertoolsBatched
                | Self::ItertoolsZipLongest
                | Self::ItertoolsCombinations
                | Self::ItertoolsCombinationsWithReplacement
                | Self::ItertoolsPermutations
                | Self::ItertoolsProduct
                | Self::ItertoolsGroupBy
                | Self::ItertoolsGrouper
                | Self::ItertoolsTee
                | Self::CsvReader
                | Self::CsvDictReader
        )
    }

    /// Checks if a value of type `self` is an instance of `other`.
    ///
    /// This handles Python's subtype relationships:
    /// - `bool` is a subtype of `int` (so `isinstance(True, int)` returns True)
    /// - `datetime` is a subtype of `date` (so `isinstance(datetime_obj, date)` returns True)
    #[must_use]
    pub fn is_instance_of(self, other: Self) -> bool {
        if self == other {
            true
        } else if other == Self::Object {
            // `object` is the universal base: every value is an instance of it,
            // even though Monty stores it in no MRO (see `types/class.rs`).
            true
        } else if self == Self::Bool && other == Self::Int {
            // bool is a subtype of int in Python
            true
        } else if self == Self::DateTime && other == Self::Date {
            // datetime is a subtype of date in Python
            true
        } else if (self == Self::DefaultDict || self == Self::Counter) && other == Self::Dict {
            // collections.defaultdict and collections.Counter subclass dict
            true
        } else if self == Self::NamedTuple && other == Self::Tuple {
            // a namedtuple class is generated as a tuple subclass
            true
        } else {
            false
        }
    }

    /// Converts a callable type to a u8 for the `CallBuiltinType` opcode.
    ///
    /// Returns `Some(u8)` for types that can be called as constructors,
    /// `None` for non-callable types.
    #[must_use]
    pub fn callable_to_u8(self) -> Option<u8> {
        match self {
            Self::Bool => Some(0),
            Self::Int => Some(1),
            Self::Float => Some(2),
            Self::Str => Some(3),
            Self::Bytes => Some(4),
            Self::List => Some(5),
            Self::Tuple => Some(6),
            Self::Dict => Some(7),
            Self::Set => Some(8),
            Self::FrozenSet => Some(9),
            Self::Range => Some(10),
            Self::Slice => Some(11),
            Self::Iterator => Some(12),
            Self::Path => Some(13),
            Self::Complex => Some(14),
            _ => None,
        }
    }

    /// Converts a u8 back to a callable `Type` for the `CallBuiltinType` opcode.
    ///
    /// Returns `Some(Type)` for valid callable type IDs, `None` otherwise.
    #[must_use]
    pub fn callable_from_u8(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Bool),
            1 => Some(Self::Int),
            2 => Some(Self::Float),
            3 => Some(Self::Str),
            4 => Some(Self::Bytes),
            5 => Some(Self::List),
            6 => Some(Self::Tuple),
            7 => Some(Self::Dict),
            8 => Some(Self::Set),
            9 => Some(Self::FrozenSet),
            10 => Some(Self::Range),
            11 => Some(Self::Slice),
            12 => Some(Self::Iterator),
            13 => Some(Self::Path),
            14 => Some(Self::Complex),
            _ => None,
        }
    }

    /// Dispatches classmethod calls on builtin type objects (e.g. `dict.fromkeys`).
    ///
    /// Keeps classmethod behavior centralized with type semantics instead of VM
    /// call plumbing. Any other name is looked up as a plain attribute and the
    /// result called, so `list.__name__()` raises `'str' object is not callable`.
    pub(crate) fn call_class_method(
        self,
        method_id: StringId,
        args: ArgValues,
        vm: &mut VM<'_>,
    ) -> RunResult<CallResult> {
        match (self, vm.interns.static_string(method_id)) {
            // Type-level `dict.fromkeys(...)`, so the result is a plain dict.
            (Self::Dict, Some(StaticStrings::Fromkeys)) => {
                dict_fromkeys(args, DictKind::plain(), vm).map(CallResult::Value)
            }
            // `defaultdict.fromkeys(...)` builds `cls()`, i.e. a defaultdict with no
            // factory — matching CPython's inherited `dict.fromkeys` classmethod.
            (Self::DefaultDict, Some(StaticStrings::Fromkeys)) => {
                dict_fromkeys(args, DictKind::defaultdict(None), vm).map(CallResult::Value)
            }
            // `chain.from_iterable(iterable)`, CPython's one classmethod here.
            (Self::ItertoolsChain, Some(StaticStrings::FromIterable)) => {
                itertools::call(vm, ItertoolsFunctions::ChainFromIterable, args).map(CallResult::Value)
            }
            // Counter deliberately disables the inherited classmethod.
            (Self::Counter, Some(StaticStrings::Fromkeys)) => {
                args.drop_with(vm);
                Err(ExcType::not_implemented("Counter.fromkeys() is undefined.  Use Counter(iterable) instead.").into())
            }
            (Self::Bytes, Some(StaticStrings::Fromhex)) => bytes_fromhex(args, vm).map(CallResult::Value),
            (Self::Complex, Some(StaticStrings::FromNumber)) => {
                complex::class_from_number(vm, args).map(CallResult::Value)
            }
            (Self::Decimal, Some(StaticStrings::FromNumber)) => {
                decimal::py::class_from_number(vm, args).map(CallResult::Value)
            }
            (Self::Decimal, _) if vm.interns.get_str(method_id) == "from_float" => {
                decimal::py::class_from_float(vm, args).map(CallResult::Value)
            }
            (Self::Date, Some(StaticStrings::Today)) => date::class_today(vm, args),
            (Self::Path, Some(StaticStrings::Cwd)) => path::class_cwd(vm, args).map(CallResult::Value),
            (Self::Date, Some(StaticStrings::Fromisoformat)) => {
                date::class_fromisoformat(vm.heap, args, vm.interns).map(CallResult::Value)
            }
            (Self::DateTime, Some(StaticStrings::Now)) => datetime::class_now(vm, args),
            (Self::DateTime, Some(StaticStrings::Strptime)) => {
                datetime::class_strptime(vm.heap, args, vm.interns).map(CallResult::Value)
            }
            (Self::DateTime, Some(StaticStrings::Fromisoformat)) => {
                datetime::class_fromisoformat(vm.heap, args, vm.interns).map(CallResult::Value)
            }
            // `object.__setattr__(obj, name, value)` called directly, which is
            // how it is nearly always reached; `object.__setattr__` as a value
            // is handled by `class_getattr`.
            (Self::Object, _) if vm.interns.get_str(method_id) == "__setattr__" => {
                builtin_object_setattr(vm, args).map(CallResult::Value)
            }
            (Self::DateTime, Some(StaticStrings::Combine)) => datetime::class_combine(vm, args).map(CallResult::Value),
            (Self::Time, Some(StaticStrings::Fromisoformat)) => {
                time::class_fromisoformat(vm, args).map(CallResult::Value)
            }
            // `list.__class_getitem__(int)` is `list[int]`; the error names the
            // bare type as CPython does (`deque.__class_getitem__()`).
            (ty, Some(StaticStrings::ClassGetitem)) if ty.has_class_getitem() => {
                let name = format!("{}.__class_getitem__", ty.dunder_name(vm.heap, vm.interns));
                let key = args.get_one_arg(&name, vm.heap)?;
                Ok(CallResult::Value(GenericAlias::subscript(
                    ty.try_into().expect("generic origins are builtin types"),
                    key,
                    vm,
                )))
            }
            // The type's plain attributes (`list.__name__`); a missing name
            // raises the lookup's `type object 'list' has no attribute` error.
            _ => {
                let attr = EitherStr::Interned(method_id);
                if let Some(value) = self.class_getattr(&attr, vm) {
                    defer_drop!(value, vm);
                    vm.call_function(value, args)
                } else {
                    args.drop_with(vm.heap);
                    Err(self.attribute_error(&attr, vm))
                }
            }
        }
    }

    /// Resolves an attribute read on a builtin type object itself — `list.__name__`,
    /// `date.max`, `chain.from_iterable` — returning `None` when there is no such
    /// member so the caller raises [`Self::attribute_error`].
    pub(crate) fn class_getattr(self, attr: &EitherStr, vm: &mut VM<'_>) -> Option<Value> {
        let is_dunder_name = attr.static_string(vm.interns).map_or_else(
            || attr.as_str(vm.interns) == "__name__",
            |ss| ss == StaticStrings::DunderName,
        );
        if is_dunder_name {
            Some(allocate_string(self.dunder_name(vm.heap, vm.interns), vm.heap))
        } else if let Some(constant) = self.class_constant(attr, vm) {
            Some(constant)
        } else if self == Self::ItertoolsChain && attr.static_string(vm.interns) == Some(StaticStrings::FromIterable) {
            // `chain.from_iterable`, the one attribute an `itertools` type
            // carries. Handed out as a value so it can be bound and called
            // later, not only called in place.
            Some(Value::ModuleFunction(ModuleFunctions::Itertools(
                ItertoolsFunctions::ChainFromIterable,
            )))
        } else if self == Self::Object && attr.as_str(vm.interns) == "__setattr__" {
            // `object.__setattr__` is the only member `object` carries: it
            // exists so a class that hooks attribute writes has a way to
            // perform one (see `limitations/classes.md`).
            Some(Value::Builtin(Builtins::Function(BuiltinsFunctions::ObjectSetattr)))
        } else {
            None
        }
    }

    /// The `AttributeError` for a name missing from this type object.
    ///
    /// CPython names the class rather than the metaclass here:
    /// `type object 'list' has no attribute 'nonexistent'`.
    pub(crate) fn attribute_error(self, attr: &EitherStr, vm: &VM<'_>) -> RunError {
        ExcType::attribute_error_type(&self.name(vm.heap, vm.interns), attr.as_str(vm.interns))
    }

    /// Resolves a class-level constant on a builtin type object (`time.max`,
    /// `timezone.utc`, ...), returning `None` so the caller can fall through to
    /// its own `AttributeError`.
    ///
    /// Every lookup allocates a fresh object, so `date.min is date.min` is
    /// `False` where CPython caches (see limitations/datetime.md).
    pub(crate) fn class_constant(self, attr: &EitherStr, vm: &mut VM<'_>) -> Option<Value> {
        // One microsecond short of `MAX_TIMEDELTA_DAYS + 1` days, which
        // normalizes to CPython's `timedelta(days=999999999, seconds=86399,
        // microseconds=999999)`.
        const MAX_TIMEDELTA_MICROS: i128 = ((MAX_TIMEDELTA_DAYS as i128) + 1) * DAY_MICROSECONDS - 1;
        const MIN_TIMEDELTA_MICROS: i128 = (MIN_TIMEDELTA_DAYS as i128) * DAY_MICROSECONDS;

        Some(match (self, attr.static_string(vm.interns)?) {
            (Self::Date, StaticStrings::Min) => date::allocate_ymd(1, 1, 1, vm.heap),
            (Self::Date, StaticStrings::Max) => date::allocate_ymd(9999, 12, 31, vm.heap),
            (Self::Date, StaticStrings::Resolution) => timedelta::allocate_micros(DAY_MICROSECONDS, vm.heap),
            (Self::DateTime, StaticStrings::Min) => datetime::allocate_naive(1, 1, 1, 0, 0, 0, 0, vm.heap),
            (Self::DateTime, StaticStrings::Max) => {
                datetime::allocate_naive(9999, 12, 31, 23, 59, 59, 999_999, vm.heap)
            }
            (Self::Time, StaticStrings::Min) => time::allocate_naive(0, 0, 0, 0, vm.heap),
            (Self::Time, StaticStrings::Max) => time::allocate_naive(23, 59, 59, 999_999, vm.heap),
            (Self::TimeDelta, StaticStrings::Min) => timedelta::allocate_micros(MIN_TIMEDELTA_MICROS, vm.heap),
            (Self::TimeDelta, StaticStrings::Max) => timedelta::allocate_micros(MAX_TIMEDELTA_MICROS, vm.heap),
            // The three microsecond-resolution classes share one constant.
            (Self::DateTime | Self::Time | Self::TimeDelta, StaticStrings::Resolution) => {
                timedelta::allocate_micros(1, vm.heap)
            }
            (Self::TimeZone, StaticStrings::Utc) => vm.heap.get_timezone_utc(),
            // The `_blake2` size limits, on the class and so on its instances.
            (Self::Blake2b, StaticStrings::MaxDigestSize | StaticStrings::MaxKeySize) => Value::Int(64),
            (Self::Blake2b, StaticStrings::SaltSize | StaticStrings::PersonSize) => Value::Int(16),
            (Self::Blake2s, StaticStrings::MaxDigestSize | StaticStrings::MaxKeySize) => Value::Int(32),
            (Self::Blake2s, StaticStrings::SaltSize | StaticStrings::PersonSize) => Value::Int(8),
            (Self::TimeZone, StaticStrings::Min) => timezone::allocate_offset(-MAX_TIMEZONE_CONSTANT_SECONDS, vm.heap),
            (Self::TimeZone, StaticStrings::Max) => timezone::allocate_offset(MAX_TIMEZONE_CONSTANT_SECONDS, vm.heap),
            _ => return None,
        })
    }

    /// Calls this type as a constructor (e.g., `list(x)`, `int(x)`).
    ///
    /// Iterator acquisition may suspend in user code; other constructors complete natively.
    pub(crate) fn call(self, vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult> {
        if self == Self::Iterator {
            builtin_iter(vm, args)
        } else {
            self.call_native(vm, args).map(CallResult::Value)
        }
    }

    /// Dispatches constructors which do not yield, including primitive conversions.
    fn call_native(self, vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
        match self {
            // Container types - delegate to init methods
            Self::List => List::init(vm, args),
            Self::Deque => Deque::init(vm, args),
            Self::Tuple => Tuple::init(vm, args),
            Self::Dict => Dict::init(vm, args),
            Self::DefaultDict => collections::defaultdict_init(vm, args),
            Self::Counter => collections::counter_init(vm, args),
            Self::Set => Set::init(vm, args),
            Self::FrozenSet => FrozenSet::init(vm, args),
            Self::Str => Str::init(vm, args),
            Self::Bytes => Bytes::init(vm, args),
            Self::Range => Range::init(vm, args),
            Self::Slice => Slice::init(vm, args),
            Self::Date => date::init(vm, args),
            Self::DateTime => datetime::init(vm, args),
            Self::Time => time::init(vm, args),
            Self::TimeDelta => timedelta::init(vm, args),
            Self::TimeZone => TimeZone::init(vm, args),
            Self::Path => Path::init(vm, args),
            Self::Partial => Partial::init(vm, args),
            Self::Random => Random::init(vm, args),
            Self::Blake2b => hashlib::blake2_init(Blake2Kind::Blake2b, vm, args),
            Self::Blake2s => hashlib::blake2_init(Blake2Kind::Blake2s, vm, args),

            // Every `itertools` name but `tee` is a type, as in CPython, so
            // `isinstance(x, itertools.count)` and `type(x) is count` hold.
            Self::ItertoolsCount
            | Self::ItertoolsRepeat
            | Self::ItertoolsPairwise
            | Self::ItertoolsCompress
            | Self::ItertoolsIslice
            | Self::ItertoolsChain
            | Self::ItertoolsCycle
            | Self::ItertoolsTakeWhile
            | Self::ItertoolsDropWhile
            | Self::ItertoolsFilterFalse
            | Self::ItertoolsStarMap
            | Self::ItertoolsAccumulate
            | Self::ItertoolsBatched
            | Self::ItertoolsZipLongest
            | Self::ItertoolsCombinations
            | Self::ItertoolsCombinationsWithReplacement
            | Self::ItertoolsPermutations
            | Self::ItertoolsProduct
            | Self::ItertoolsGroupBy
            | Self::ItertoolsGrouper
            | Self::ItertoolsTee
            | Self::ItertoolsTeeDataObject => itertools::construct(self, vm, args),

            Self::Complex => complex::init(vm, args),
            Self::CsvDictReader => csv::dict_reader_init(vm, args),
            Self::CsvDictWriter => csv::dict_writer_init(vm, args),
            Self::CsvDialect => csv::dialect_init(vm, args),
            Self::Decimal => decimal::py::init(vm, args),
            Self::DecimalContext => decimal::py_context::init(vm, args),

            // Primitive types - inline implementation
            Self::Int => int_init(vm, args),
            Self::Float => {
                let interns = vm.interns;
                let Some(v) = args.get_zero_one_arg("float", vm.heap)? else {
                    return Ok(Value::Float(0.0));
                };
                defer_drop!(v, vm);
                if let Some(value) = v.py_float(vm)? {
                    Ok(Value::Float(value))
                } else {
                    match v {
                        Value::InternString(string_id) => {
                            Ok(Value::Float(parse_f64_from_str(interns.get_str(*string_id))?))
                        }
                        Value::Ref(heap_id) if let HeapData::Str(s) = vm.heap.get(*heap_id) => {
                            Ok(Value::Float(parse_f64_from_str(s.as_str())?))
                        }
                        _ => Err(ExcType::type_error_float_conversion(&v.py_type_name(vm))),
                    }
                }
            }
            Self::Bool => {
                let Some(v) = args.get_zero_one_arg("bool", vm.heap)? else {
                    return Ok(Value::Bool(false));
                };
                defer_drop!(v, vm);
                Ok(Value::Bool(v.py_bool(vm)?))
            }

            // CPython words this one differently from the other uncallable types.
            Self::Union => {
                args.drop_with(vm);
                Err(ExcType::type_error("cannot create 'typing.Union' instances"))
            }

            // Non-callable types - raise TypeError
            _ => {
                args.drop_with(vm.heap);
                Err(ExcType::type_error_not_callable(&self.name(vm.heap, vm.interns)))
            }
        }
    }
}

/// Parses a Python `float()` string argument into an `f64`.
///
/// This supports:
/// - Leading/trailing whitespace (e.g. `"  1.5  "`)
/// - The special values `inf`, `-inf`, `infinity`, and `nan` (case-insensitive)
/// - Underscore digit separators, strictly between two digits (`1_000.5`, `1e1_0`)
pub(crate) fn parse_f64_from_str(value: &str) -> RunResult<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(value_error_could_not_convert_string_to_float(value));
    }
    let stripped;
    let trimmed = if trimmed.contains('_') {
        stripped = strip_underscores(trimmed).ok_or_else(|| value_error_could_not_convert_string_to_float(value))?;
        stripped.as_str()
    } else {
        trimmed
    };

    let lower = trimmed.to_ascii_lowercase();
    let parsed = match lower.as_str() {
        "inf" | "+inf" | "infinity" | "+infinity" => f64::INFINITY,
        "-inf" | "-infinity" => f64::NEG_INFINITY,
        "nan" | "+nan" => f64::NAN,
        "-nan" => -f64::NAN,
        _ => trimmed
            .parse::<f64>()
            .map_err(|_| value_error_could_not_convert_string_to_float(value))?,
    };

    Ok(parsed)
}

/// Removes digit-separator underscores, or returns `None` when one is not
/// strictly between two digits (CPython's `_Py_string_to_number_with_underscores`).
pub(crate) fn strip_underscores(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.char_indices() {
        if c == '_' {
            let before = i.checked_sub(1).map(|j| bytes[j]);
            let after = bytes.get(i + 1);
            if !(before.is_some_and(|b| b.is_ascii_digit()) && after.is_some_and(u8::is_ascii_digit)) {
                return None;
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

/// Creates the `ValueError` raised by `float()` when a string cannot be parsed.
///
/// Matches CPython's message format: `could not convert string to float: '...'`.
fn value_error_could_not_convert_string_to_float(value: &str) -> RunError {
    SimpleException::new_msg(
        ExcType::ValueError,
        format!("could not convert string to float: {}", StringRepr(value)),
    )
    .into()
}

/// Argument shape for `int(x=..., /, base=...)`. `x` is positional-only with
/// no kwarg id, so `int(x=1)` reports an unknown keyword exactly like CPython;
/// `vectorcall` + `at_most_total` model `long_vectorcall`'s dual arity wording.
#[derive(FromArgs)]
#[from_args(name = "int", at_most_total, vectorcall)]
struct IntArgs {
    #[from_args(pos_only, default)]
    x: Option<Value>,
    #[from_args(default)]
    base: Option<Value>,
}

/// Implements the `int()` constructor: numeric coercion, and str/bytes
/// parsing with an optional base (auto-detected when `base=0`).
fn int_init(vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
    let IntArgs { x, base } = IntArgs::from_args(args, vm)?;
    let Some(x) = x else {
        // `int()` → 0; `int(base=N)` complains about the missing value even
        // before validating the base, matching `long_new_impl`'s ordering.
        return match base {
            None => Ok(Value::Int(0)),
            Some(base) => {
                base.drop_with(vm);
                Err(ExcType::type_error_int_missing_string_argument())
            }
        };
    };
    defer_drop!(x, vm);
    match base {
        None => int_convert(x, vm),
        Some(base) => {
            let base = int_base(base, vm)?;
            let interns = vm.interns;
            match x {
                Value::InternString(string_id) => parse_int_from_str(interns.get_str(*string_id), base, vm.heap),
                Value::InternBytes(bytes_id) => parse_int_from_bytes(interns.get_bytes(*bytes_id), base, vm.heap),
                Value::Ref(heap_id) => match vm.heap.get(*heap_id) {
                    HeapData::Str(s) => parse_int_from_str(s.as_str(), base, vm.heap),
                    HeapData::Bytes(b) => parse_int_from_bytes(b.as_slice(), base, vm.heap),
                    _ => Err(ExcType::type_error_int_non_string_with_base()),
                },
                _ => Err(ExcType::type_error_int_non_string_with_base()),
            }
        }
    }
}

/// `int(x)` with no base: numeric coercion plus base-10 str/bytes parsing.
fn int_convert(x: &Value, vm: &mut VM<'_>) -> RunResult<Value> {
    if let Some(value) = x.py_int_impl(vm)? {
        Ok(value)
    } else if let Some(value) = x.py_index_impl(vm)? {
        Ok(value)
    } else {
        match x.py_value(vm) {
            PyValue::Str(value) => parse_int_from_str(value.as_str(vm), 10, vm.heap),
            PyValue::Bytes(value) => parse_int_from_bytes(value.as_slice(vm), 10, vm.heap),
            _ => Err(ExcType::type_error_int_conversion(&x.py_type_name(vm))),
        }
    }
}

/// Resolves the `base` argument to `0` or `2..=36`, consuming the value.
///
/// Mirrors CPython: the base goes through `PyNumber_AsSsize_t(obase, NULL)`,
/// which *clamps* out-of-i64 ints instead of raising — so a `LongInt` base
/// lands in the range error, not `OverflowError`; non-integers raise
/// `TypeError` before the range is checked.
fn int_base(base: Value, vm: &mut VM<'_>) -> RunResult<u32> {
    defer_drop!(base, vm);
    let n = base.saturating_index(vm)?;
    match u32::try_from(n) {
        Ok(0) => Ok(0),
        Ok(b @ 2..=36) => Ok(b),
        _ => Err(ExcType::value_error_int_base_range()),
    }
}

/// Parses a Python `int()` string argument into an `Int` or `LongInt`.
///
/// `base` is `0` (auto-detect from a `0x`/`0o`/`0b` prefix) or `2..=36`.
/// Returns `Value::Int` if the value fits in i64, otherwise allocates a
/// `LongInt` on the heap. Returns `ValueError` on failure.
fn parse_int_from_str(value: &str, base: u32, heap: &Heap) -> RunResult<Value> {
    // Fast path: plain base-10 literals parse directly (no whitespace,
    // underscores or prefix handling needed).
    if base == 10
        && let Ok(int) = value.parse::<i64>()
    {
        return Ok(Value::Int(int));
    }
    let trimmed = value.trim();
    // Preserve the allocation-free path for whitespace-padded i64 values
    // without retrying unchanged inputs that already failed above.
    if base == 10
        && trimmed.len() != value.len()
        && let Ok(int) = trimmed.parse::<i64>()
    {
        Ok(Value::Int(int))
    } else {
        let invalid = || ExcType::value_error_invalid_literal_for_int(base, StringRepr(value));
        parse_int_digits(trimmed, base, &invalid, heap)
    }
}

/// Parses a Python `int()` bytes argument using ASCII whitespace rules.
///
/// Unlike `str`, bytes must not treat UTF-8 encodings of Unicode whitespace as
/// separators. Failures repr the input as a bytes literal, matching CPython.
fn parse_int_from_bytes(bytes: &[u8], base: u32, heap: &Heap) -> RunResult<Value> {
    let invalid = || ExcType::value_error_invalid_literal_for_int(base, bytes_repr(bytes));
    match str::from_utf8(bytes.trim_ascii()) {
        Ok(s) => parse_int_digits(s, base, &invalid, heap),
        Err(_) => Err(invalid()),
    }
}

/// Tracks what the previous character was while scanning an int literal, to
/// enforce CPython's underscore rules: `_` allowed only between digits or
/// directly after a base prefix, never leading, trailing, or doubled.
enum IntScanState {
    Start,
    Prefix,
    Digit,
    Underscore,
}

/// Parses a whitespace-trimmed str/bytes int literal: sign, base prefix,
/// underscore placement, digit limits, and BigInt promotion.
fn parse_int_digits(value: &str, base: u32, invalid: &impl Fn() -> RunError, heap: &Heap) -> RunResult<Value> {
    let (negative, body) = match value.strip_prefix(['+', '-']) {
        Some(rest) => (value.starts_with('-'), rest),
        None => (false, value),
    };

    // Resolve the effective base and strip any `0x`/`0o`/`0b` prefix. For
    // `base=0` a leading zero *without* a prefix is only legal if every digit
    // is zero (CPython rejects `010` as an ambiguous octal-looking literal).
    let bytes = body.as_bytes();
    let mut digits = body;
    let mut effective_base = if base == 0 { 10 } else { base };
    let mut error_if_nonzero = false;
    if bytes.first() == Some(&b'0') {
        let prefix_base = match bytes.get(1) {
            Some(b'x' | b'X') => Some(16),
            Some(b'o' | b'O') => Some(8),
            Some(b'b' | b'B') => Some(2),
            Some(_) if base == 0 => {
                error_if_nonzero = true;
                None
            }
            _ => None,
        };
        if let Some(prefix_base) = prefix_base
            && (base == 0 || base == prefix_base)
        {
            effective_base = prefix_base;
            digits = &body[2..];
        }
    }
    let had_prefix = digits.len() != body.len();

    // Validate digits and underscore placement, collecting the cleaned digits
    // (no underscores) behind the sign. Untracked `String`, but bounded by the
    // input which is itself an already-tracked string.
    let mut cleaned = String::with_capacity(usize::from(negative) + digits.len());
    if negative {
        cleaned.push('-');
    }
    let mut state = if had_prefix {
        IntScanState::Prefix
    } else {
        IntScanState::Start
    };
    for c in digits.chars() {
        if c == '_' {
            if !matches!(state, IntScanState::Digit | IntScanState::Prefix) {
                return Err(invalid());
            }
            state = IntScanState::Underscore;
        } else if c.is_digit(effective_base) {
            cleaned.push(c);
            state = IntScanState::Digit;
        } else {
            return Err(invalid());
        }
    }
    // Must end on a digit: rejects empty input, a bare prefix/sign, and
    // trailing underscores in one check.
    if !matches!(state, IntScanState::Digit) {
        return Err(invalid());
    }
    if error_if_nonzero && cleaned.bytes().any(|b| !matches!(b, b'0' | b'-')) {
        return Err(invalid());
    }

    if let Ok(int) = i64::from_str_radix(&cleaned, effective_base) {
        return Ok(Value::Int(int));
    }
    // CPython's int↔str digit limit applies only to bases that are not a
    // power of two (where conversion cost is linear, not quadratic).
    let digit_count = cleaned.len() - usize::from(negative);
    if !effective_base.is_power_of_two() && digit_count > INT_MAX_STR_DIGITS {
        return Err(ExcType::value_error_int_str_too_large(digit_count));
    }
    match BigInt::parse_bytes(cleaned.as_bytes(), effective_base) {
        Some(bi) => Ok(LongInt::new(bi).into_value(heap)),
        // Unreachable in practice: every char was validated as a digit above.
        None => Err(invalid()),
    }
}

#[cfg(test)]
mod builtin_type_tests {
    use std::mem::size_of;

    use strum::VariantNames;

    use super::{BuiltinType, ExcType, HeapId, Type};
    use crate::{builtins::Builtins, value::Value};

    #[test]
    fn static_type_conversions_preserve_serialization() {
        let types = Type::VARIANTS
            .iter()
            .filter(|name| !matches!(**name, "instance" | "exception"))
            .map(|name| name.parse::<Type>().unwrap());
        let exceptions = ExcType::VARIANTS
            .iter()
            .map(|name| Type::Exception(name.parse().unwrap()));
        assert_eq!(BuiltinType::VARIANTS.len() + 1, Type::VARIANTS.len());

        for ty in types.chain(exceptions) {
            let builtin = BuiltinType::try_from(ty).unwrap();
            assert_eq!(Type::from(builtin), ty);
            assert_eq!(builtin.to_string(), ty.to_string());

            let cbor = minicbor_serde::to_vec(ty).unwrap();
            assert_eq!(minicbor_serde::to_vec(builtin).unwrap(), cbor);
            assert_eq!(minicbor_serde::from_slice::<BuiltinType>(&cbor).unwrap(), builtin);

            // id() encodes builtins through postcard, whose variant indices must also stay stable.
            let postcard = postcard::to_allocvec(&ty).unwrap();
            assert_eq!(postcard::to_allocvec(&builtin).unwrap(), postcard);
            assert_eq!(postcard::from_bytes::<BuiltinType>(&postcard).unwrap(), builtin);
        }
    }

    // Captured from Type before the split. Updating this fixture requires checking
    // dump compatibility and id() stability, not just regenerating expected output.
    #[test]
    fn builtin_type_serialization_matches_legacy_format() {
        let fixture = include_str!("../../tests/fixtures/builtin_type_serialization.txt");
        assert_eq!(fixture.lines().count() + 1, BuiltinType::VARIANTS.len());

        for line in fixture.lines() {
            let (name, index) = line.split_once(' ').unwrap();
            let cbor = minicbor_serde::to_vec(name).unwrap();
            let builtin: BuiltinType = minicbor_serde::from_slice(&cbor).unwrap();

            assert_eq!(minicbor_serde::to_vec(builtin).unwrap(), cbor, "{name}");
            let postcard = vec![index.parse::<u8>().unwrap()];
            assert_eq!(postcard::to_allocvec(&builtin).unwrap(), postcard, "{name}");
            assert_eq!(postcard::from_bytes::<BuiltinType>(&postcard).unwrap(), builtin);
        }

        // Exception carries a payload and follows the removed Instance slot.
        let cbor = b"\xa1\x69Exception\x6aValueError";
        let builtin = BuiltinType::Exception(ExcType::ValueError);
        assert_eq!(minicbor_serde::to_vec(builtin).unwrap(), cbor);
        assert_eq!(minicbor_serde::from_slice::<BuiltinType>(cbor).unwrap(), builtin);
        let postcard = [29, 17];
        assert_eq!(postcard::to_allocvec(&builtin).unwrap(), postcard);
        assert_eq!(postcard::from_bytes::<BuiltinType>(&postcard).unwrap(), builtin);
    }

    #[test]
    fn instance_types_cannot_be_builtins() {
        let instance = Type::Instance(HeapId::from_index(0));
        assert!(BuiltinType::try_from(instance).is_err());

        let cbor = minicbor_serde::to_vec(instance).unwrap();
        assert!(minicbor_serde::from_slice::<BuiltinType>(&cbor).is_err());

        let postcard = postcard::to_allocvec(&instance).unwrap();
        assert!(postcard::from_bytes::<BuiltinType>(&postcard).is_err());
    }

    #[test]
    fn compact_builtin_layout() {
        assert_eq!(size_of::<BuiltinType>(), 1);
        assert_eq!(size_of::<Builtins>(), 2);

        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<Value>(), 16);
    }
}
