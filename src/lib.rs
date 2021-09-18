use cc;
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
    host: String,
    is_force: bool,
}

impl Builder {
    pub fn default() -> Builder {
        let out_dir = env::var_os("OUT_DIR").unwrap().into_string().unwrap();
        let target = env::var("TARGET").ok().unwrap();
        let host = env::var("HOST").ok().unwrap();
        Builder::new(&out_dir, &target, &host, false)
    }

    pub fn new(out_dir: &str, target: &str, host: &str, is_force: bool) -> Builder {
        let base_dir = PathBuf::from(out_dir.to_owned()).join("tassl-build");
        Builder {
            build_dir: base_dir.join("build"),
            install_dir: base_dir.join("install"),
            target: target.to_owned(),
            host: host.to_owned(),
            is_force
        }
    }

    #[cfg(target_os = "linux")]
    fn get_configure(&self) -> Command {
        let mut configure = Command::new("sh");
        configure.arg("./config");
        configure.arg(&format!("--prefix={}", self.install_dir.display()));
        configure
    }

    #[cfg(target_os = "macos")]
    fn get_configure(&self) -> Command {
        let perl_program = env::var("OPENSSL_SRC_PERL").unwrap_or(
            env::var("PERL").unwrap_or("perl".to_string())
        );
        let mut configure = Command::new(perl_program);
        configure
            .arg("./Configure")
            .arg(&format!("--prefix={}", self.install_dir.display()))
            // No shared objects, we just want static libraries
            .arg("no-dso")
            .arg("no-shared")
            // No need to build tests, we won't run them anyway
            .arg("no-tests")
            // Nothing related to zlib please
            .arg("no-comp")
            .arg("no-zlib")
            .arg("no-zlib-dynamic")
            // Avoid multilib-postfix for build targets that specify it
            .arg("--libdir=lib")
            // No support for multiple providers yet
            .arg("no-legacy");

        let os = match self.target.as_str() {
            "aarch64-apple-darwin" => "darwin64-arm64-cc",
            "i686-apple-darwin" => "darwin-i386-cc",
            "x86_64-apple-darwin" => "darwin64-x86_64-cc",
            _ => panic!("Don't know how to configure TASSL for {}", &self.target),
        };
        configure.arg(os);

        let mut cc = cc::Build::new();
        cc.target(&self.target).host(&self.host).warnings(false).opt_level(2);
        let compiler = cc.get_compiler();
        configure.env("CC", compiler.path());
        let path = compiler.path().to_str().unwrap();

        // Both `cc::Build` and `./Configure` take into account
        // `CROSS_COMPILE` environment variable. So to avoid double
        // prefix, we unset `CROSS_COMPILE` for `./Configure`.
        configure.env_remove("CROSS_COMPILE");

        // Infer ar/ranlib tools from cross compilers if the it looks like
        // we're doing something like `foo-gcc` route that to `foo-ranlib`
        // as well.
        if path.ends_with("-gcc") {
            let path = &path[..path.len() - 4];
            if env::var_os("RANLIB").is_none() {
                configure.env("RANLIB", format!("{}-ranlib", path));
            }
            if env::var_os("AR").is_none() {
                configure.env("AR", format!("{}-ar", path));
            }
        }

        // Make sure we pass extra flags like `-ffunction-sections` and
        // other things like ARM codegen flags.
        let mut skip_next = false;
        for arg in compiler.args() {
            // cc includes an `-arch` flag for Apple platforms, but we've
            // already selected an arch implicitly via the target above, and
            // OpenSSL contains about the conflict if both are specified.
            if self.target.contains("apple") {
                if arg == "-arch" {
                    skip_next = true;
                    continue;
                }
            }
            if skip_next {
                skip_next = false;
                continue;
            }
            configure.arg(arg);
        }
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

        let mut configure = self.get_configure();
        configure.current_dir(&current_work_dir);
        run_command(configure, "configuring TASSL build");

        let mut depend = Command::new("make");
        depend.arg("depend").current_dir(&current_work_dir);
        run_command(depend, "building TASSL dependencies");

        let mut build = Command::new("make");
        build.arg("build_libs").current_dir(&current_work_dir);
        if let Some(s) = env::var_os("CARGO_MAKEFLAGS") {
            build.env("MAKEFLAGS", s);
        }
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
