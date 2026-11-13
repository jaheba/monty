#[cfg(any(feature = "experimental-native", feature = "experimental-aot"))]
fn main() {
    use std::{env, fmt::Write, fs, path::PathBuf, process::Command};

    use object::{Object, ObjectSection, ObjectSymbol};

    println!("cargo:rustc-check-cfg=cfg(monty_native)");

    for path in [
        "build.rs",
        "src/bytecode/native/abi.rs",
        "src/bytecode/native/stencils.rs",
        "src/bytecode/aot/abi.rs",
        "src/bytecode/aot/stencils.rs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    if !(target.starts_with("aarch64-") || target.starts_with("x86_64-")) {
        fs::write(
            out.join("native_stencils.rs"),
            "const STENCILS: &[(&str, &[u8])] = &[];",
        )
        .unwrap();
        return;
    }

    println!("cargo:rustc-cfg=monty_native");

    let mut tiers = Vec::new();
    if cfg!(feature = "experimental-native") {
        tiers.push(("native", 34));
    }
    if cfg!(feature = "experimental-aot") {
        tiers.push(("aot", 46));
    }
    for (tier, expected) in tiers {
        let source =
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join(format!("src/bytecode/{tier}/stencils.rs"));
        let object_path = out.join(format!("{tier}_stencils.o"));
        let llvm_path = out.join(format!("{tier}_stencils.ll"));
        let status = Command::new(env::var_os("RUSTC").unwrap())
            .current_dir(&out)
            .arg(source)
            .args([
                "--crate-name",
                "monty_stencils",
                "--crate-type",
                "lib",
                "--edition=2024",
                "--target",
                &target,
                "-Copt-level=3",
                "-Cpanic=abort",
                "-Cdebug-assertions=no",
                "-Coverflow-checks=no",
                "-Ccodegen-units=1",
                "-Clto=off",
                "--emit",
            ])
            .arg(format!("obj={},llvm-ir={}", object_path.display(), llvm_path.display()))
            .status()
            .expect("compile native stencils");
        assert!(status.success(), "native stencil compilation failed");

        let bytes = fs::read(object_path).unwrap();
        let file = object::File::parse(bytes.as_slice()).unwrap();
        let ir = fs::read_to_string(llvm_path).unwrap();
        let functions = file
            .symbols()
            .filter(|symbol| symbol.is_definition() && symbol.kind() == object::SymbolKind::Text)
            .collect::<Vec<_>>();
        let mut generated = String::from("const STENCILS: &[(&str, &[u8])] = &[\n");
        let mut count = 0;
        let mut declarations = String::from("unsafe extern \"C\" {\n");
        let mut shared = String::from("const SHARED_STENCILS: &[(&str, Entry)] = &[\n");

        for symbol in &functions {
            let name = symbol.name().unwrap().trim_start_matches('_');
            let Some(short_name) = name.strip_prefix("monty_stencil_") else {
                continue;
            };
            let section = file.section_by_index(symbol.section_index().unwrap()).unwrap();
            let end = if symbol.size() == 0 {
                functions
                    .iter()
                    .filter(|next| next.section_index() == symbol.section_index() && next.address() > symbol.address())
                    .map(object::ObjectSymbol::address)
                    .min()
                    .unwrap_or(section.address() + section.size())
            } else {
                symbol.address() + symbol.size()
            };
            let start = symbol.address() - section.address();
            let size = end - symbol.address();
            assert!(size > 0, "empty stencil {name}");
            for (offset, relocation) in section.relocations() {
                assert!(
                    offset < start || offset >= start + size,
                    "external relocation in {name}: {relocation:?}"
                );
            }

            if short_name != "exit" && short_name != "return" {
                let mut marker = format!("@{name}(");
                if !ir.contains(&marker) {
                    let alias = ir
                        .lines()
                        .find(|line| line.starts_with(&format!("@{name} =")))
                        .expect("stencil alias");
                    let target = alias.rsplit('@').next().unwrap().split_whitespace().next().unwrap();
                    marker = format!("@{target}(");
                }
                let body = &ir[ir.find(&marker).expect("stencil LLVM definition")..];
                let body = &body[..body.find("\n}").unwrap()];
                let continuations = body
                    .lines()
                    .filter(|line| line.contains(" call ") && line.contains(" i64 %") && !line.contains('@'))
                    .collect::<Vec<_>>();
                assert!(!continuations.is_empty(), "missing continuation in {name}");
                assert!(
                    continuations
                        .iter()
                        .all(|line| line.contains("tail call") || line.contains("musttail call")),
                    "non-tail continuation in {name}"
                );
            }

            let code =
                &section.data().unwrap()[usize::try_from(start).unwrap()..usize::try_from(start + size).unwrap()];
            writeln!(generated, "({short_name:?}, &{code:?}),").unwrap();
            writeln!(
                declarations,
                "fn {name}(context: *mut super::abi::Context, patch: *const Patch) -> u64;"
            )
            .unwrap();
            writeln!(shared, "({short_name:?}, {name}),").unwrap();
            count += 1;
        }

        assert_eq!(count, expected, "unexpected stencil set");
        generated.push_str("];\n");
        fs::write(out.join(format!("{tier}_stencils.rs")), generated).unwrap();
        #[cfg(feature = "experimental-aot")]
        if tier == "aot" {
            use ar_archive_writer::{ArchiveKind, DEFAULT_OBJECT_READER, NewArchiveMember, write_archive_to_stream};

            let kind = if target.contains("windows") {
                ArchiveKind::Coff
            } else if target.contains("apple") {
                ArchiveKind::Darwin
            } else {
                ArchiveKind::Gnu
            };
            let members = [NewArchiveMember::new(
                bytes.as_slice(),
                &DEFAULT_OBJECT_READER,
                "aot_stencils.o".to_owned(),
            )];
            let archive_name = if target.ends_with("-msvc") {
                "monty_aot_stencils.lib"
            } else {
                "libmonty_aot_stencils.a"
            };
            let mut archive = fs::File::create(out.join(archive_name)).unwrap();
            write_archive_to_stream(&mut archive, &members, kind, false, None).unwrap();
            declarations.push_str("}\n");
            shared.push_str("];\n");
            declarations.push_str(&shared);
            fs::write(out.join("aot_shared_stencils.rs"), declarations).unwrap();
            println!("cargo:rustc-link-search=native={}", out.display());
            println!("cargo:rustc-link-lib=static=monty_aot_stencils");
        }
    }
}

#[cfg(not(any(feature = "experimental-native", feature = "experimental-aot")))]
fn main() {
    println!("cargo:rustc-check-cfg=cfg(monty_native)");
}
