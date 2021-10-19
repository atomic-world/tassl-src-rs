use std::{env, fs, path::{Path, PathBuf}, process::Command};

fn run_command(mut command: Command, desc: &str) {
    println!("running {:?}", command);
    let status = command.status().unwrap();
    if !status.success() {
        panic!(
            "
Error {}:
    Command: {:?}
    Exit status: {}
    ",
            desc, command, status
        );
    }
}

fn cp_r(src: &Path, dst: &Path) {
    for f in fs::read_dir(src).unwrap() {
        let f = f.unwrap();
        let path = f.path();
        let name = path.file_name().unwrap();

        // Skip git metadata as it's been known to cause issues (#26) and
        // otherwise shouldn't be required
        if name.to_str() == Some(".git") {
            continue;
        }

        let dst = dst.join(name);
        if f.file_type().unwrap().is_dir() {
            fs::create_dir_all(&dst).unwrap();
            cp_r(&path, &dst);
        } else {
            let _ = fs::remove_file(&dst);
            fs::copy(&path, &dst).unwrap();
        }
    }
}


pub struct Artifacts {
    pub include_dir: PathBuf,
    pub lib_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub libs: Vec<String>,
}

impl Artifacts {
    pub fn print_cargo_metadata(&self) {
        println!("cargo:rustc-link-search=native={}", self.lib_dir.display());
        for lib in self.libs.iter() {
            println!("cargo:rustc-link-lib=static={}", lib);
        }
        println!("cargo:include={}", self.include_dir.display());
        println!("cargo:lib={}", self.lib_dir.display());
    }
}

pub struct Builder {
    build_dir: PathBuf,
    install_dir: PathBuf,
    target: String,
    is_force: bool,
}

impl Builder {
    pub fn default() -> Builder {
        let out_dir = env::var_os("OUT_DIR").unwrap().into_string().unwrap();
        let target = env::var("TARGET").ok().unwrap();
        Builder::new(&out_dir, &target, false)
    }

    pub fn new(out_dir: &str, target: &str, is_force: bool) -> Builder {
        let base_dir = PathBuf::from(out_dir.to_owned()).join("tassl-build");
        Builder {
            build_dir: base_dir.join("build"),
            install_dir: base_dir.join("install"),
            target: target.to_owned(),
            is_force
        }
    }

    #[cfg(target_os = "linux")]
    fn get_configure(&self) -> Command {
        let mut configure = Command::new("sh");
        configure.arg("./config");
        configure.arg(&format!("--prefix={}", self.install_dir.display()));
        configure.arg("-DOPENSSL_PIC");
        configure.arg("no-shared");
        configure
    }

    #[cfg(target_os = "macos")]
    fn get_configure(&self) -> Command {
        let mut configure = Command::new("sh");
        configure.arg("./Configure");
        configure.arg(&format!("--prefix={}", self.install_dir.display()));
        let os = match self.target.as_str() {
            "aarch64-apple-darwin" => "darwin64-arm64-cc",
            "i686-apple-darwin" => "darwin-i386-cc",
            "x86_64-apple-darwin" => "darwin64-x86_64-cc",
            _ => panic!("Don't know how to configure TASSL for {}", &self.target),
        };
        configure.arg(os);
        configure
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "linux")))]
    pub fn build(&self) -> Artifacts {
        panic!("Not support {:?} yet.", self.target);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn build(&self) -> Artifacts {
        if self.install_dir.exists() {
            if self.is_force {
                fs::remove_dir_all(&self.install_dir).unwrap();
            } else {
                return Artifacts {
                    lib_dir: self.install_dir.join("lib"),
                    bin_dir: self.install_dir.join("bin"),
                    include_dir: self.install_dir.join("include"),
                    libs: vec!["ssl".to_string(), "crypto".to_string()],
                };
            }
        }

        if self.build_dir.exists() {
            fs::remove_dir_all(&self.build_dir).unwrap();
        }

        let current_work_dir = self.build_dir.join("src");
        fs::create_dir_all(&current_work_dir).unwrap();

        let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("TASSL");
        cp_r(&source_dir, &current_work_dir);

        let mut chmod = Command::new("chmod");
        chmod.current_dir(&current_work_dir);
        chmod.arg("-R").arg("a+x").arg("./util");
        run_command(chmod, "change TASSL util permission");

        let mut configure = self.get_configure();
        configure.current_dir(&current_work_dir);
        run_command(configure, "configuring TASSL");

        let mut build = Command::new("make");
        build.current_dir(&current_work_dir);
        run_command(build, "building TASSL");

        let mut install = Command::new("make");
        install.arg("install").current_dir(&current_work_dir);
        run_command(install, "installing TASSL");

        fs::remove_dir_all(&current_work_dir).unwrap();
        Artifacts {
            lib_dir: self.install_dir.join("lib"),
            bin_dir: self.install_dir.join("bin"),
            include_dir: self.install_dir.join("include"),
            libs: vec!["ssl".to_string(), "crypto".to_string()],
        }
    }
}
