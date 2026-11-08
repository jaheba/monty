//! Python `pathlib.Path` type implementation.
//!
//! Provides a path object with both pure methods (no I/O) and filesystem methods
//! (require `OsAccess` implementation). Pure methods are handled directly by the VM,
//! while filesystem methods yield external function calls for the host to resolve.

pub(crate) mod home;
mod segments;
mod uri;

use std::{
    cell::Cell,
    collections::hash_map::DefaultHasher,
    fmt::Write,
    hash::{Hash, Hasher},
    iter::repeat_n,
};

use monty_types::{MontyPath, ResourceTracker, StringRepr};
use smallvec::SmallVec;

use crate::{
    args::{ArgValues, FromArgs, LaxBool},
    builtins::open::builtin_open,
    bytecode::{CallResult, VM},
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult, SimpleException},
    hash::HashValue,
    heap::{DropWithContext, Heap, HeapData, HeapId, HeapItem, HeapObjectRead, HeapReadOutput},
    intern::{Interns, StaticStrings},
    os_dispatch::{build_path_os_call, is_path_os_method, value_as_path_str},
    resource_checks::collect_checked,
    string_builder::StringBuilder,
    types::{
        LazyHeapSet, List, PyTrait, Type, allocate_tuple,
        dir_scan::{glob_call, walk_call},
        str::allocate_string,
    },
    value::{EitherStr, Value},
};

mod pattern;

/// Python `pathlib.Path` object representing a filesystem path.
///
/// Stores a normalized POSIX path string. Windows-style paths are converted
/// to POSIX format (backslashes to forward slashes).
///
/// The path is immutable - all operations that would modify the path return
/// new `Path` objects or strings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Path {
    /// The normalized path string.
    path: String,
    /// Lazily-computed Python hash. Paths are immutable (all "modifying"
    /// operations return new `Path` objects). Skipped on serde — see
    /// [`super::Str::cached_hash`] for the rationale.
    #[serde(skip)]
    cached_hash: Cell<Option<HashValue>>,
}

impl PartialEq for Path {
    /// Compares only the path string — `cached_hash` is a pure optimisation.
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Path {
    /// Creates a new `Path` from a path string.
    ///
    /// The path is normalized:
    /// - Backslashes are converted to forward slashes
    /// - Trailing slashes are preserved for root paths only
    #[must_use]
    pub fn new(path: String) -> Self {
        Self {
            path: normalize_path(path),
            cached_hash: Cell::new(None),
        }
    }

    /// Returns the path as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Returns the final component of the path.
    ///
    /// Returns an empty string for `.`, the root, or an empty path.
    #[must_use]
    pub fn name(&self) -> &str {
        if self.path == "." {
            ""
        } else {
            self.path.rsplit_once('/').map_or(self.path.as_str(), |(_, name)| name)
        }
    }

    /// Returns the path without its final component (parent directory).
    ///
    /// For relative paths without a directory (like `file.txt`), returns `.`.
    /// Returns `None` only for the root path `/`.
    #[must_use]
    pub fn parent(&self) -> Option<&str> {
        if self.path == "/" {
            return None;
        }
        match self.path.rsplit_once('/') {
            Some((parent, _)) => Some(if parent.is_empty() { "/" } else { parent }),
            None => Some("."), // Relative path without directory component
        }
    }

    /// Returns the final component without its last suffix.
    ///
    /// If the name has multiple suffixes (e.g., "file.tar.gz"), only the
    /// last suffix is removed.
    #[must_use]
    pub fn stem(&self) -> &str {
        let name = self.name();
        if name.starts_with('.') && !name[1..].contains('.') {
            // Hidden file without extension (e.g., ".bashrc")
            return name;
        }
        name.rsplit_once('.').map_or(name, |(stem, _)| stem)
    }

    /// Returns the file extension (last suffix), including the leading dot.
    ///
    /// Returns an empty string if there is no extension.
    #[must_use]
    pub fn suffix(&self) -> &str {
        let name = self.name();
        if name.starts_with('.') && !name[1..].contains('.') {
            // Hidden file without extension (e.g., ".bashrc")
            return "";
        }
        name.rfind('.').map_or("", |idx| &name[idx..])
    }

    /// Returns all file extensions as a list of strings.
    ///
    /// Each suffix includes its leading dot. Returns an empty list if no extensions.
    #[must_use]
    pub fn suffixes(&self) -> Vec<&str> {
        let name = self.name();
        if name.is_empty() || name == "." || name == ".." {
            return Vec::new();
        }

        let start_idx = usize::from(name.starts_with('.'));
        let search_str = &name[start_idx..];

        let mut result = Vec::new();
        let mut pos = 0;
        while let Some(idx) = search_str[pos..].find('.') {
            let abs_idx = pos + idx;
            // Each suffix is from this dot to the end or next dot
            let suffix_end = search_str[abs_idx + 1..]
                .find('.')
                .map_or(search_str.len(), |next| abs_idx + 1 + next);
            result.push(&name[start_idx + abs_idx..start_idx + suffix_end]);
            pos = abs_idx + 1;
        }
        result
    }

    /// Returns the path components as a list of strings.
    ///
    /// Absolute paths start with "/" as the first component; `.` has no parts.
    #[must_use]
    pub fn parts(&self) -> Vec<&str> {
        if self.path.is_empty() || self.path == "." {
            return Vec::new();
        }

        let mut parts = Vec::new();
        if self.path.starts_with('/') {
            parts.push("/");
            let rest = &self.path[1..];
            if !rest.is_empty() {
                parts.extend(rest.split('/').filter(|s| !s.is_empty()));
            }
        } else {
            parts.extend(self.path.split('/').filter(|s| !s.is_empty()));
        }
        parts
    }

    /// Returns `true` if the path is absolute (starts with `/`).
    #[must_use]
    pub fn is_absolute(&self) -> bool {
        self.path.starts_with('/')
    }

    /// Collects components while bounding native traversal and temporary storage.
    fn checked_parts(&self, tracker: &ResourceTracker) -> RunResult<Vec<&str>> {
        if self.path == "." {
            return Ok(Vec::new());
        }
        let anchor = self.is_absolute().then_some("/");
        let components = self.path.split('/').filter(|part| !part.is_empty());
        Ok(collect_checked(anchor.into_iter().chain(components), tracker)?)
    }

    /// Tests lexical containment by components, without resolving `..` or symlinks.
    pub fn is_relative_to(&self, other: &Self, tracker: &ResourceTracker) -> RunResult<bool> {
        if self.is_absolute() != other.is_absolute() {
            return Ok(false);
        }
        let parts = self.checked_parts(tracker)?;
        let other_parts = other.checked_parts(tracker)?;
        if parts.len() < other_parts.len() {
            return Ok(false);
        }
        for (index, other_part) in other_parts.iter().enumerate() {
            tracker.check_time_every(index)?;
            if parts.get(index) != Some(other_part) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Builds a relative path, rejecting any `..` component that would have to be walked upwards.
    pub fn relative_to(&self, other: &Self, walk_up: bool, tracker: &ResourceTracker) -> RunResult<String> {
        let parts = self.checked_parts(tracker)?;
        let other_parts = other.checked_parts(tracker)?;
        let same_anchor = self.is_absolute() == other.is_absolute();
        let mut common = 0;
        for (index, (part, other_part)) in parts.iter().zip(&other_parts).enumerate() {
            tracker.check_time_every(index)?;
            if part != other_part {
                break;
            }
            common += 1;
        }

        if !(walk_up || same_anchor && common == other_parts.len()) {
            return Err(ExcType::value_error(format!(
                "{} is not in the subpath of {}",
                StringRepr(self.as_str()),
                StringRepr(other.as_str())
            )));
        }

        // A shared `..` prefix is valid; crossing one assumes filesystem semantics we cannot infer lexically.
        for (index, part) in other_parts[common..].iter().enumerate() {
            tracker.check_time_every(index)?;
            if *part == ".." {
                return Err(ExcType::value_error(format!(
                    "'..' segment in {} cannot be walked",
                    StringRepr(other.as_str())
                )));
            }
        }

        if !same_anchor {
            return Err(ExcType::value_error(format!(
                "{} and {} have different anchors",
                StringRepr(self.as_str()),
                StringRepr(other.as_str())
            )));
        }

        let mut result = StringBuilder::new(tracker);
        let upward = other_parts.len() - common;
        for (index, part) in repeat_n("..", upward)
            .chain(parts[common..].iter().copied())
            .enumerate()
        {
            tracker.check_time_every(index)?;
            if index != 0 {
                result.push('/')?;
            }
            result.push_str(part)?;
        }
        if upward == 0 && parts.len() == common {
            result.push('.')?;
        }
        result.finish_raw()
    }

    /// Joins this path with another path component.
    ///
    /// If `other` is an absolute path, it replaces `self` entirely.
    #[must_use]
    pub fn joinpath(&self, other: &str) -> String {
        if other.starts_with('/') || self.path.is_empty() || self.path == "." {
            normalize_path(other.to_owned())
        } else if self.path.ends_with('/') {
            normalize_path(format!("{}{}", self.path, other))
        } else {
            normalize_path(format!("{}/{}", self.path, other))
        }
    }

    /// Returns a new path with the name changed.
    ///
    /// # Errors
    /// Returns an error if the path has no name or if the new name is empty.
    pub fn with_name(&self, name: &str) -> Result<String, String> {
        if name.is_empty() {
            return Err("Invalid name: empty string".to_owned());
        }
        if name.contains('/') {
            return Err(format!("Invalid name: {name:?} contains path separator"));
        }
        if self.name().is_empty() {
            return Err("Path has no name".to_owned());
        }

        if let Some(parent) = self.parent() {
            if parent == "/" {
                Ok(format!("/{name}"))
            } else if parent == "." {
                // Relative path without directory - just use the new name
                Ok(name.to_owned())
            } else {
                Ok(format!("{parent}/{name}"))
            }
        } else {
            Ok(name.to_owned())
        }
    }

    /// Returns a new path with the stem changed (keeps the suffix).
    ///
    /// # Errors
    /// Returns an error if the path has no name or if the new stem is empty.
    pub fn with_stem(&self, stem: &str) -> Result<String, String> {
        if stem.is_empty() {
            return Err("Invalid stem: empty string".to_owned());
        }
        if stem.contains('/') {
            return Err(format!("Invalid stem: {stem:?} contains path separator"));
        }
        if self.name().is_empty() {
            return Err("Path has no name".to_owned());
        }

        let suffix = self.suffix();
        let new_name = format!("{stem}{suffix}");
        self.with_name(&new_name)
    }

    /// Returns a new path with the suffix changed.
    ///
    /// If the suffix is empty, removes the existing suffix.
    /// If the suffix doesn't start with '.', it's added.
    pub fn with_suffix(&self, suffix: &str) -> Result<String, String> {
        if self.name().is_empty() {
            return Err("Path has no name".to_owned());
        }

        let suffix = if suffix.is_empty() || suffix.starts_with('.') {
            suffix.to_owned()
        } else {
            format!(".{suffix}")
        };

        if suffix.contains('/') {
            return Err(format!("Invalid suffix: {suffix:?} contains path separator"));
        }

        let stem = self.stem();
        let new_name = format!("{stem}{suffix}");
        self.with_name(&new_name)
    }

    /// Returns the path as a POSIX string (forward slashes).
    ///
    /// Since paths are already stored in POSIX format, this just returns the path.
    #[must_use]
    pub fn as_posix(&self) -> &str {
        &self.path
    }

    /// Creates a `Path` from the `Path()` constructor call.
    ///
    /// Accepts zero or more path segments that are joined together.
    /// - `Path()` returns `Path('.')`
    /// - `Path('a')` returns `Path('a')`
    /// - `Path('a', 'b', 'c')` returns `Path('a/b/c')`
    /// - If an absolute path appears, it replaces everything before it.
    pub fn init(vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
        let pos_args = args.into_pos_only("Path", vm.heap)?;
        defer_drop!(pos_args, vm);

        let path = match pos_args.as_slice() {
            [] => {
                // No arguments, return Path('.')
                Self::new(".".to_owned())
            }
            [single] => {
                // Single argument, just convert to Path
                Self::new(extract_path_segment(single, vm)?.to_owned())
            }
            [first_arg, rest @ ..] => {
                let base = Self::new(extract_path_segment(first_arg, vm)?.to_owned());
                fold_joinpath(base, rest, vm)?
            }
        };
        Ok(Value::Ref(vm.heap.allocate(HeapData::Path(path))))
    }
}

/// Classmethod `Path.cwd()`, also callable through instances: returns the VM's
/// virtual working directory without a host round-trip.
///
/// A pure-Python classmethod in CPython, so its arity errors count `cls`:
/// one stray positional is `takes 1 positional argument but 2 were given`.
pub(crate) fn class_cwd(vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
    check_no_args(args, "Path.cwd", "cls", vm)?;
    let path = Path::new(vm.env.cwd.to_string());
    Ok(Value::Ref(vm.heap.allocate(HeapData::Path(path))))
}

/// Python's zero-argument methods count the bound receiver and report keyword conflicts first.
fn check_no_args(args: ArgValues, name: &str, receiver: &str, vm: &mut VM<'_>) -> RunResult<()> {
    if matches!(args, ArgValues::Empty) {
        return Ok(());
    }

    let (pos, kwargs) = args.into_parts();
    let given = pos.len() + 1;
    let first_kwarg = kwargs.first_key(vm.heap, vm.interns);
    pos.drop_with(vm);
    kwargs.drop_with(vm);
    Err(match first_kwarg? {
        Some(key) if key == receiver => {
            ExcType::type_error(format!("{name}() got multiple values for argument '{receiver}'"))
        }
        Some(key) => ExcType::type_error_unexpected_keyword(name, &key),
        None => ExcType::type_error(format!("{name}() takes 1 positional argument but {given} were given")),
    })
}

/// `Path.glob(pattern, *, case_sensitive=None, recurse_symlinks=False)`.
#[derive(FromArgs)]
#[from_args(name = "Path.glob", style = def)]
struct PathGlobArgs {
    #[from_args(static_string = "PatternAttr")]
    pattern: Value,
    #[from_args(kw_only, default = Value::None)]
    case_sensitive: Value,
    #[from_args(kw_only, default = LaxBool::new(false))]
    recurse_symlinks: LaxBool,
}

/// Pure-Python signatures include the bound receiver in arity errors.
#[derive(FromArgs)]
#[from_args(name = "PurePath.relative_to", style = def)]
struct PathRelativeToArgs {
    #[from_args(pos_only)]
    receiver: Value,
    other: Value,
    #[from_args(kw_only, default = Value::Bool(false))]
    walk_up: Value,
}

/// `Path.is_relative_to(other)` accepts the same path operands as construction.
#[derive(FromArgs)]
#[from_args(name = "PurePath.is_relative_to", style = def)]
struct PathIsRelativeToArgs {
    #[from_args(pos_only)]
    receiver: Value,
    other: Value,
}

/// Converts string and native path operands, preserving pathlib's argument-error wording.
fn relative_path_operand(other: &Value, vm: &VM<'_>) -> RunResult<Path> {
    // Native paths are already normalized; splitting them again can dominate the deadline.
    if let Value::Ref(id) = other
        && let HeapReadOutput::Path(path) = vm.heap.read(*id)
    {
        return Ok(path.get(vm.heap).clone());
    }

    extract_path_segment(other, vm).map(|text| Path::new(text.to_owned()))
}

/// `Path.rglob(pattern, *, case_sensitive=None, recurse_symlinks=False)`.
#[derive(FromArgs)]
#[from_args(name = "Path.rglob", style = def)]
struct PathRglobArgs {
    #[from_args(static_string = "PatternAttr")]
    pattern: Value,
    #[from_args(kw_only, default = Value::None)]
    case_sensitive: Value,
    #[from_args(kw_only, default = LaxBool::new(false))]
    recurse_symlinks: LaxBool,
}

/// `Path.walk(top_down=True, on_error=None, follow_symlinks=False)`.
#[derive(FromArgs)]
#[from_args(name = "Path.walk", style = def)]
struct PathWalkArgs {
    #[from_args(default = LaxBool::new(true))]
    top_down: LaxBool,
    #[from_args(default = Value::None)]
    on_error: Value,
    #[from_args(default = LaxBool::new(false))]
    follow_symlinks: LaxBool,
}

/// `Path.glob()` / `Path.rglob()` on the receiver `base`: validates the
/// pattern as `pathlib` does, then hands it to [`glob_call`].
///
/// `rglob` is `glob` of `'**/' + pattern`. Any non-`None` `case_sensitive` is
/// truth-tested, and makes CPython list even literal pattern parts.
fn path_glob(
    base: &str,
    pattern: Value,
    recursive: bool,
    case_sensitive: Value,
    recurse_symlinks: LaxBool,
    vm: &mut VM<'_>,
) -> RunResult<CallResult> {
    defer_drop!(pattern, vm);
    defer_drop!(case_sensitive, vm);
    let pattern_text = match value_as_path_str(pattern, vm.heap, vm.interns) {
        Some(text) => text.to_owned(),
        // CPython's `bytes` handling trips over its own `str` separator.
        None if pattern.py_type(vm) == Type::Bytes => {
            return Err(ExcType::type_error("a bytes-like object is required, not 'str'"));
        }
        None => {
            return Err(ExcType::type_error(format!(
                "_path_splitroot_ex: path should be string, bytes or os.PathLike, not {}",
                pattern.py_type_name(vm)
            )));
        }
    };
    let case_sensitive = match case_sensitive {
        Value::None => None,
        value => Some(value.py_bool(vm)?),
    };
    let pattern_text = if recursive && !pattern_text.starts_with('/') {
        format!("**/{pattern_text}")
    } else {
        pattern_text
    };
    glob_call(base, &pattern_text, case_sensitive, recurse_symlinks.bool())
}

/// Extracts a string from a Value for use as a path.
fn extract_path_string<'a>(val: &'a Value, vm: &'a VM<'_>) -> RunResult<&'a str> {
    value_as_path_str(val, vm.heap, vm.interns)
        .ok_or_else(|| ExcType::type_error(format!("expected str or Path, got {}", val.py_type_name(vm))))
}

/// Constructor and join operands share pathlib's path-like conversion errors.
fn extract_path_segment<'a>(value: &'a Value, vm: &'a VM<'_>) -> RunResult<&'a str> {
    value_as_path_str(value, vm.heap, vm.interns).ok_or_else(|| {
        ExcType::type_error(format!(
            "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
            value.py_type_name(vm)
        ))
    })
}

fn fold_joinpath(mut path: Path, parts: &[Value], vm: &VM<'_>) -> RunResult<Path> {
    for (index, part) in parts.iter().enumerate() {
        vm.heap.tracker.check_time_every(index)?;
        let part = extract_path_segment(part, vm)?;
        if part.starts_with('/') || path.path.is_empty() || path.path == "." {
            path = Path::new(part.to_owned());
        } else {
            let separator = !path.path.ends_with('/');
            // Repeated references to one segment can amplify a small argument list into a huge path.
            let mut builder = StringBuilder::from_existing(path.path, &vm.heap.tracker);
            if separator {
                builder.push('/')?;
            }
            builder.push_str(part)?;
            path = Path::new(builder.finish_raw()?);
        }
    }
    Ok(path)
}

/// Joins a `Path` with a path-like right operand for `Path.__truediv__`.
fn path_div(path: &Path, other: &Value, heap: &Heap, interns: &Interns) -> Option<Value> {
    let other = value_as_path_str(other, heap, interns)?;
    let result = path.joinpath(other);
    Some(Value::Ref(heap.allocate(HeapData::Path(Path::new(result)))))
}

/// Prepends a path-like left operand for `Path.__rtruediv__`.
fn path_rdiv(path: &Path, other: &Value, heap: &Heap, interns: &Interns) -> Option<Value> {
    let base = value_as_path_str(other, heap, interns)?;
    let result = Path::new(base.to_owned()).joinpath(path.as_str());
    Some(Value::Ref(heap.allocate(HeapData::Path(Path::new(result)))))
}

/// Normalizes a path string to POSIX format, matching CPython's `pathlib.PurePosixPath`.
///
/// - Converts backslashes to forward slashes
/// - Removes `.` components (e.g. `/a/./b` → `/a/b`)
/// - Collapses consecutive slashes (e.g. `//a///b` → `/a/b`)
/// - Removes trailing slashes (except for root "/")
/// - Turns a path with no components (`''`, `'.'`, `'./'`) into `.`
/// - Does NOT resolve `..` components (that requires I/O for symlinks)
fn normalize_path(mut path: String) -> String {
    // Convert backslashes to forward slashes
    if path.contains('\\') {
        path = path.replace('\\', "/");
    }

    // Fast path: non-empty, no `.` component and no consecutive or trailing slashes
    if !path.is_empty() && !path.contains("/.") && !path.contains("//") && !path.starts_with("./") {
        // Still strip trailing slashes
        while path.len() > 1 && path.ends_with('/') {
            path.pop();
        }
        return path;
    }

    let is_absolute = path.starts_with('/');
    let mut components: Vec<&str> = Vec::new();

    for part in path.split('/') {
        match part {
            "" | "." => {} // skip empty segments (from consecutive/trailing slashes) and "."
            other => components.push(other),
        }
    }

    if components.is_empty() {
        return if is_absolute { "/".to_owned() } else { ".".to_owned() };
    }

    let mut result = String::with_capacity(path.len());
    if is_absolute {
        result.push('/');
    }
    for (i, comp) in components.iter().enumerate() {
        if i > 0 {
            result.push('/');
        }
        result.push_str(comp);
    }
    result
}

impl Path {
    /// Resolves a known attribute by its `StaticStrings` variant.
    ///
    /// Returns `Some(value)` for recognized properties, or `None` if the variant doesn't
    /// correspond to a Path attribute. Used by `py_getattr` to share logic between
    /// the interned fast path and the heap string slow path.
    pub(crate) fn getattr_by_static(&self, ss: StaticStrings, heap: &Heap) -> Option<Value> {
        let v = match ss {
            // Sandbox paths are POSIX: no drive, and the normalized anchor is the root.
            StaticStrings::Anchor | StaticStrings::Root => {
                allocate_string(if self.is_absolute() { "/" } else { "" }, heap)
            }
            StaticStrings::Drive => allocate_string("", heap),
            StaticStrings::Name => allocate_string(self.name(), heap),
            StaticStrings::Parent => {
                if let Some(parent) = self.parent() {
                    let parent_path = Self::new(parent.to_owned());
                    Value::Ref(heap.allocate(HeapData::Path(parent_path)))
                } else {
                    // Return self when there's no parent (root or relative path)
                    let same_path = Self::new(self.as_str().to_owned());
                    Value::Ref(heap.allocate(HeapData::Path(same_path)))
                }
            }
            StaticStrings::Stem => allocate_string(self.stem(), heap),
            StaticStrings::Suffix => allocate_string(self.suffix(), heap),
            StaticStrings::Suffixes => {
                let suffixes = self.suffixes();
                let mut items = Vec::with_capacity(suffixes.len());
                for suffix in suffixes {
                    items.push(allocate_string(suffix, heap));
                }
                Value::Ref(heap.allocate(HeapData::List(List::new(items))))
            }
            StaticStrings::Parts => {
                let parts = self.parts();
                let mut items = SmallVec::with_capacity(parts.len());
                for part in parts {
                    items.push(allocate_string(part, heap));
                }
                allocate_tuple(items, heap)
            }
            _ => return None,
        };
        Some(v)
    }
}

impl<'h> PyTrait<'h> for HeapObjectRead<'h, Path> {
    fn py_type(&self, _vm: &VM<'h>) -> Type {
        Type::Path
    }

    fn py_len(&self, _vm: &VM<'h>) -> Option<usize> {
        // Paths don't have a length in Python
        None
    }

    fn py_eq_impl(&self, other: &Value, vm: &mut VM<'h>) -> RunResult<Option<bool>> {
        let Some(HeapReadOutput::Path(other)) = other.read_heap(vm) else {
            return Ok(None);
        };
        Ok(Some(self.get(vm.heap).path == other.get(vm.heap).path))
    }

    fn py_hash(&self, vm: &mut VM<'h>) -> RunResult<Option<HashValue>> {
        let p = self.get(vm.heap);
        if let Some(cached) = p.cached_hash.get() {
            return Ok(Some(cached));
        }
        let mut hasher = DefaultHasher::new();
        p.as_str().hash(&mut hasher);
        let hash = HashValue::new(hasher.finish());
        p.cached_hash.set(Some(hash));
        Ok(Some(hash))
    }

    fn py_bool(&self, _vm: &mut VM<'h>) -> RunResult<bool> {
        // Paths are always truthy (even empty paths)
        Ok(true)
    }

    fn py_truediv_impl(&self, other: &Value, vm: &mut VM<'h>) -> RunResult<Option<Value>> {
        Ok(path_div(self.get(vm.heap), other, vm.heap, vm.interns))
    }

    fn py_rtruediv_impl(&self, other: &Value, vm: &mut VM<'h>) -> RunResult<Option<Value>> {
        Ok(path_rdiv(self.get(vm.heap), other, vm.heap, vm.interns))
    }

    fn py_str(&self, vm: &mut VM<'h>) -> RunResult<Value> {
        Ok(allocate_string(self.get(vm.heap).as_str(), vm.heap))
    }

    fn py_repr_fmt(&self, f: &mut impl Write, vm: &mut VM<'h>, _heap_ids: &mut LazyHeapSet) -> RunResult<()> {
        // Format like: PosixPath('/usr/bin')
        Ok(write!(f, "PosixPath('{}')", self.get(vm.heap).path)?)
    }

    /// Handles attribute calls on Path objects, including both pure methods (no I/O)
    /// and OS methods that require host system access.
    ///
    /// OS methods yield calls to the host; `iterdir` retains the receiver's spelling
    /// so the resume path can restore it on each returned entry.
    /// Pure methods (is_absolute, joinpath, etc.) are handled directly.
    fn py_call_attr(&mut self, vm: &mut VM<'h>, attr: &EitherStr, args: ArgValues) -> RunResult<CallResult> {
        let Some(method) = attr.static_string(vm.interns) else {
            args.drop_with(vm);
            return Err(ExcType::attribute_error(Type::Path, attr.as_str(vm.interns)));
        };

        if is_path_os_method(method) {
            let path = MontyPath::new(self.get(vm.heap).as_str().to_owned());
            return build_path_os_call(method, path, args, vm);
        }

        // Pure methods (no I/O)
        let value = match method {
            StaticStrings::Cwd => class_cwd(vm, args),
            StaticStrings::Match => pattern::path_match(&self.get(vm.heap).clone(), args, vm),
            StaticStrings::Home => return home::class_home(vm, args),
            StaticStrings::Expanduser => return home::expanduser(self, vm, args),
            StaticStrings::WithSegments => segments::call(args, vm),
            StaticStrings::Glob => {
                let PathGlobArgs {
                    pattern,
                    case_sensitive,
                    recurse_symlinks,
                } = PathGlobArgs::from_args(args, vm)?;
                let base = self.get(vm.heap).as_str().to_owned();
                return path_glob(&base, pattern, false, case_sensitive, recurse_symlinks, vm);
            }
            StaticStrings::Rglob => {
                let PathRglobArgs {
                    pattern,
                    case_sensitive,
                    recurse_symlinks,
                } = PathRglobArgs::from_args(args, vm)?;
                let base = self.get(vm.heap).as_str().to_owned();
                return path_glob(&base, pattern, true, case_sensitive, recurse_symlinks, vm);
            }
            StaticStrings::Walk => {
                let PathWalkArgs {
                    top_down,
                    on_error,
                    follow_symlinks,
                } = PathWalkArgs::from_args(args, vm)?;
                let top = self.get(vm.heap).as_str().to_owned();
                return walk_call(top, true, top_down.bool(), on_error, follow_symlinks.bool(), vm);
            }
            StaticStrings::IsAbsolute => {
                args.check_zero_args("is_absolute", vm.heap)?;
                Ok(Value::Bool(self.get(vm.heap).is_absolute()))
            }
            StaticStrings::RelativeTo => {
                let PathRelativeToArgs {
                    receiver,
                    other,
                    walk_up,
                } = PathRelativeToArgs::from_args(args.prepend(Value::None), vm)?;
                receiver.drop_with(vm);
                defer_drop!(other, vm);
                defer_drop!(walk_up, vm);
                let other = relative_path_operand(other, vm)?;
                // CPython never tests walk_up's truth value when the path is already contained.
                let walk_up = !self.get(vm.heap).is_relative_to(&other, &vm.heap.tracker)? && walk_up.py_bool(vm)?;
                let result = self.get(vm.heap).relative_to(&other, walk_up, &vm.heap.tracker)?;
                Ok(Value::Ref(vm.heap.allocate(HeapData::Path(Path::new(result)))))
            }
            StaticStrings::IsRelativeTo => {
                let PathIsRelativeToArgs { receiver, other } =
                    PathIsRelativeToArgs::from_args(args.prepend(Value::None), vm)?;
                receiver.drop_with(vm);
                defer_drop!(other, vm);
                let other = relative_path_operand(other, vm)?;
                Ok(Value::Bool(self.get(vm.heap).is_relative_to(&other, &vm.heap.tracker)?))
            }
            StaticStrings::Joinpath => {
                let pos_args = args.into_pos_only("joinpath", vm.heap)?;
                defer_drop!(pos_args, vm);
                let path = fold_joinpath(self.get(vm.heap).clone(), pos_args.as_slice(), vm)?;
                Ok(Value::Ref(vm.heap.allocate(HeapData::Path(path))))
            }
            StaticStrings::WithName => {
                let name_val = args.get_one_arg("with_name", vm.heap)?;
                defer_drop!(name_val, vm);
                let name = extract_path_string(name_val, vm)?.to_owned();
                let result = self
                    .get(vm.heap)
                    .with_name(&name)
                    .map_err(|e| SimpleException::new_msg(ExcType::ValueError, &e))?;
                Ok(Value::Ref(vm.heap.allocate(HeapData::Path(Path::new(result)))))
            }
            StaticStrings::WithStem => {
                let stem_val = args.get_one_arg("with_stem", vm.heap)?;
                defer_drop!(stem_val, vm);
                let stem = extract_path_string(stem_val, vm)?.to_owned();
                let result = self
                    .get(vm.heap)
                    .with_stem(&stem)
                    .map_err(|e| SimpleException::new_msg(ExcType::ValueError, &e))?;
                Ok(Value::Ref(vm.heap.allocate(HeapData::Path(Path::new(result)))))
            }
            StaticStrings::WithSuffix => {
                let suffix_val = args.get_one_arg("with_suffix", vm.heap)?;
                defer_drop!(suffix_val, vm);
                let suffix = extract_path_string(suffix_val, vm)?.to_owned();
                let result = self
                    .get(vm.heap)
                    .with_suffix(&suffix)
                    .map_err(|e| SimpleException::new_msg(ExcType::ValueError, &e))?;
                Ok(Value::Ref(vm.heap.allocate(HeapData::Path(Path::new(result)))))
            }
            StaticStrings::AsPosix | StaticStrings::Fspath => {
                args.check_zero_args(method.into(), vm.heap)?;
                Ok(allocate_string(self.get(vm.heap).as_posix(), vm.heap))
            }
            StaticStrings::AsUri => {
                check_no_args(args, "Path.as_uri", "self", vm)?;
                let uri = uri::as_uri(self.get(vm.heap), &vm.heap.tracker)?;
                Ok(allocate_string(uri, vm.heap))
            }
            StaticStrings::Open => {
                // `Path.open(mode='r', ...)` is `open(self, mode, ...)` with
                // `self` prepended as the implicit `file` argument. Reuses
                // `builtin_open`'s mode/kwarg validation (including rejection
                // of `+`/`x` modes and non-default `buffering`/`encoding`/
                // `errors`/`newline`) so the two entry points stay in sync.
                //
                // The `inc_ref` is required because the prepended `Value::Ref`
                // is dropped by `builtin_open` via `defer_drop!` once the path
                // string has been extracted, balancing the refcount.
                let args = args.prepend(self.clone_value(vm.heap));
                return builtin_open(vm, args);
            }
            _ => {
                args.drop_with(vm);
                return Err(ExcType::attribute_error(Type::Path, attr.as_str(vm.interns)));
            }
        };
        value.map(CallResult::Value)
    }

    fn py_getattr(&self, attr: &EitherStr, vm: &mut VM<'h>) -> RunResult<Option<CallResult>> {
        // Fast path: interned strings can be matched by ID without string comparison
        if let Some(ss) = attr.static_string(vm.interns) {
            if let Some(v) = self.get(vm.heap).getattr_by_static(ss, vm.heap) {
                return Ok(Some(CallResult::Value(v)));
            }
            return Err(ExcType::attribute_error(Type::Path, attr.as_str(vm.interns)));
        }
        // Slow path: heap-allocated strings need string comparison
        let attr_str = attr.as_str(vm.interns);
        let ss = match attr_str {
            "anchor" => StaticStrings::Anchor,
            "drive" => StaticStrings::Drive,
            "root" => StaticStrings::Root,
            "name" => StaticStrings::Name,
            "parent" => StaticStrings::Parent,
            "stem" => StaticStrings::Stem,
            "suffix" => StaticStrings::Suffix,
            "suffixes" => StaticStrings::Suffixes,
            "parts" => StaticStrings::Parts,
            _ => return Err(ExcType::attribute_error(Type::Path, attr_str)),
        };
        let v = self
            .get(vm.heap)
            .getattr_by_static(ss, vm.heap)
            .expect("matched attribute must produce a value");
        Ok(Some(CallResult::Value(v)))
    }
}

impl HeapItem for Path {
    fn py_dec_ref_ids(&mut self, _stack: &mut Vec<HeapId>) {
        // Path doesn't contain heap references, nothing to do
    }
}
