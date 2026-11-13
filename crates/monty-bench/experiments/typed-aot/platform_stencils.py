"""Compile current Windows templates with Rust 1.96 and link SDK-free PE fixtures."""
import json
import os
import re
import subprocess
from pathlib import Path

study = Path(__file__).resolve().parents[5]
out = Path(__file__).resolve().parent
toolchain = study / 'msrv-toolchains/toolchains/1.96.0-aarch64-apple-darwin'
compiler = toolchain / 'bin/rustc'
linker = toolchain / 'lib/rustlib/aarch64-apple-darwin/bin/rust-lld'
scripts = [p for p in (study / 'msrv-target/debug/build').glob('monty-*/build-script-build')
           if re.fullmatch('monty-[0-9a-f]+', p.parent.name)]
script = max(scripts, key=lambda p: p.stat().st_mtime_ns)
records = []
for target, machine in [('x86_64-pc-windows-msvc', 'x64'), ('aarch64-pc-windows-msvc', 'arm64')]:
    work = study / 'stencil-platforms-unified' / target
    work.mkdir(parents=True, exist_ok=False)
    env = os.environ | {'RUSTC': str(compiler), 'TARGET': target, 'OUT_DIR': str(work),
                        'CARGO_MANIFEST_DIR': str(study / 'msrv-source/crates/monty')}
    with (out / f'unified-{target}-stencils.log').open('w') as log:
        build = subprocess.run([str(script)], env=env, stdout=log, stderr=subprocess.STDOUT)
    assert build.returncode == 0, target
    names = re.findall(r'fn (monty_stencil_\w+)\(', (work / 'aot_shared_stencils.rs').read_text())
    assert len(names) == 46, names
    command = [str(linker), '-flavor', 'link', '/dll', '/noentry', '/nodefaultlib',
               f'/machine:{machine}', f'/out:{work / "stencils.dll"}',
               f'/wholearchive:{work / "monty_aot_stencils.lib"}',
               *[f'/export:{name}' for name in names]]
    if machine == 'x64':
        marker = work / 'float_marker.rs'
        marker.write_text('#![no_std]\n#[unsafe(no_mangle)]\npub static _fltused: i32 = 0;\n')
        subprocess.run([str(compiler), str(marker), '--crate-type', 'lib', '--edition', '2024',
                        '--target', target, '-Cpanic=abort', '--emit', 'obj', '-o', str(work / 'float_marker.o')], check=True)
        command.append(str(work / 'float_marker.o'))
    with (out / f'unified-{target}-link.log').open('w') as log:
        linked = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    records.append({'target': target, 'build_script': str(script), 'compiler': str(compiler),
                    'build_exit': build.returncode, 'link_exit': linked.returncode,
                    'requested_exports': len(names), 'archive_bytes': (work / 'monty_aot_stencils.lib').stat().st_size,
                    'boundary': 'SDK-free template fixture; x86 provides the CRT _fltused marker. No Windows runtime execution.'})
    assert linked.returncode == 0, target
(out / 'unified-windows-stencil-checks.json').write_text(json.dumps(records, indent=2) + '\n')
print(json.dumps(records))
