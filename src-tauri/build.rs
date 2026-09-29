use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    tauri_build::build();
    println!("cargo:rerun-if-env-changed=JEV_STANDALONE_ASSET_DIR");

    if env::var_os("CARGO_FEATURE_STANDALONE").is_some() {
        generate_standalone_assets();
    }
}

fn generate_standalone_assets() {
    let root = env::var_os("JEV_STANDALONE_ASSET_DIR")
        .map(PathBuf::from)
        .expect("standalone feature requires JEV_STANDALONE_ASSET_DIR");
    let root = root
        .canonicalize()
        .expect("JEV_STANDALONE_ASSET_DIR must exist");
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));

    for required in ["jev-switch-daemon.exe", "ui/dist/index.html"] {
        assert!(
            files.iter().any(|(path, _)| path == required),
            "standalone bundle is missing {required}"
        );
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let output = out_dir.join("standalone_assets.rs");
    let mut file = fs::File::create(&output).expect("create generated standalone asset table");
    writeln!(
        file,
        "#[derive(Clone, Copy)] pub struct EmbeddedAsset {{ pub path: &'static str, pub bytes: &'static [u8] }}"
    )
    .expect("write generated asset type");
    writeln!(file, "pub static EMBEDDED_ASSETS: &[EmbeddedAsset] = &[")
        .expect("write generated asset table");

    for (relative, source) in files {
        println!("cargo:rerun-if-changed={}", source.display());
        let source = source.canonicalize().expect("canonicalize embedded asset");
        writeln!(
            file,
            "    EmbeddedAsset {{ path: {:?}, bytes: include_bytes!({:?}) }},",
            relative,
            source.to_string_lossy()
        )
        .expect("write generated asset entry");
    }

    writeln!(file, "]; ").expect("finish generated asset table");
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) {
    let entries = fs::read_dir(directory).expect("read standalone asset directory");
    for entry in entries {
        let entry = entry.expect("read standalone asset entry");
        let file_type = entry.file_type().expect("read standalone asset type");
        if file_type.is_symlink() {
            panic!("standalone bundle must not contain symbolic links");
        }

        let path = entry.path();
        if file_type.is_dir() {
            collect_files(root, &path, files);
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("asset remains under bundle root")
                .to_str()
                .expect("standalone asset path must be valid Unicode")
                .replace('\\', "/");
            files.push((relative, path));
        } else {
            panic!("standalone bundle contains an unsupported filesystem entry");
        }
    }
}
