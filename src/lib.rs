use cc;
use std::{env, fs, path::{Path, PathBuf}, process::Command};

pub struct Build {
    out_dir: Option<PathBuf>,
    target: Option<String>,
    host: Option<String>,
}

pub struct Artifacts {
    include_dir: PathBuf,
    lib_dir: PathBuf,
    bin_dir: PathBuf,
    libs: Vec<String>,
    target: String,
}

impl Build {
    pub fn new() -> Build {
        Build {
            out_dir: env::var_os("OUT_DIR").map(|s| PathBuf::from(s).join("tassl-build")),
            target: env::var("TARGET").ok(),
            host: env::var("HOST").ok(),
        }
    }

    pub fn out_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Build {
        self.out_dir = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn target(&mut self, target: &str) -> &mut Build {
        self.target = Some(target.to_string());
        self
    }

    pub fn host(&mut self, host: &str) -> &mut Build {
        self.host = Some(host.to_string());
        self
    }

    fn cmd_make(&self) -> Command {
        let host = &self.host.as_ref().expect("HOST dir not set")[..];
        if host.contains("dragonfly")
            || host.contains("freebsd")
            || host.contains("openbsd")
            || host.contains("solaris")
            || host.contains("illumos")
        {
            Command::new("gmake")
        } else {
            Command::new("make")
        }
    }

    pub fn build(&mut self) -> Artifacts {
        let target = &self.target.as_ref().expect("TARGET dir not set")[..];
        let host = &self.host.as_ref().expect("HOST dir not set")[..];
        let out_dir = self.out_dir.as_ref().expect("OUT_DIR not set");
        let build_dir = out_dir.join("build");
        let install_dir = out_dir.join("install");

        if build_dir.exists() {
            fs::remove_dir_all(&build_dir).unwrap();
        }
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).unwrap();
        }

        let inner_dir = build_dir.join("src");
        fs::create_dir_all(&inner_dir).unwrap();

        let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("TASSl");
        cp_r(&source_dir, &inner_dir);

        let perl_program = env::var("OPENSSL_SRC_PERL").unwrap_or(
            env::var("PERL").unwrap_or("perl".to_string())
        );
        let mut configure = Command::new(perl_program);
        configure
            .arg("./Configure")
            .arg(&format!("--prefix={}", install_dir.display()))
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

        if target.contains("musl") {
            // This actually fails to compile on musl (it needs linux/version.h
            // right now) but we don't actually need this most of the time.
            // API.
            configure.arg("no-engine");
        }

        if target.contains("musl") {
            // MUSL doesn't implement some of the libc functions that the async
            // stuff depends on, and we don't bind to any of that in any case.
            configure.arg("no-async");
        }

        let os = match target {
            "aarch64-apple-darwin" => "darwin64-arm64-cc",
            "aarch64-unknown-freebsd" => "BSD-generic64",
            "aarch64-unknown-linux-gnu" => "linux-aarch64",
            "aarch64-unknown-linux-musl" => "linux-aarch64",
            "arm-unknown-linux-gnueabi" => "linux-armv4",
            "arm-unknown-linux-gnueabihf" => "linux-armv4",
            "arm-unknown-linux-musleabi" => "linux-armv4",
            "arm-unknown-linux-musleabihf" => "linux-armv4",
            "armv5te-unknown-linux-gnueabi" => "linux-armv4",
            "armv5te-unknown-linux-musleabi" => "linux-armv4",
            "armv6-unknown-freebsd" => "BSD-generic32",
            "armv7-unknown-freebsd" => "BSD-generic32",
            "armv7-unknown-linux-gnueabi" => "linux-armv4",
            "armv7-unknown-linux-musleabi" => "linux-armv4",
            "armv7-unknown-linux-gnueabihf" => "linux-armv4",
            "armv7-unknown-linux-musleabihf" => "linux-armv4",
            "i586-unknown-linux-gnu" => "linux-elf",
            "i586-unknown-linux-musl" => "linux-elf",
            "i686-apple-darwin" => "darwin-i386-cc",
            "i686-unknown-freebsd" => "BSD-x86-elf",
            "i686-unknown-linux-gnu" => "linux-elf",
            "i686-unknown-linux-musl" => "linux-elf",
            "mips-unknown-linux-gnu" => "linux-mips32",
            "mips-unknown-linux-musl" => "linux-mips32",
            "mips64-unknown-linux-gnuabi64" => "linux64-mips64",
            "mips64-unknown-linux-muslabi64" => "linux64-mips64",
            "mips64el-unknown-linux-gnuabi64" => "linux64-mips64",
            "mips64el-unknown-linux-muslabi64" => "linux64-mips64",
            "mipsel-unknown-linux-gnu" => "linux-mips32",
            "mipsel-unknown-linux-musl" => "linux-mips32",
            "powerpc-unknown-freebsd" => "BSD-generic32",
            "powerpc-unknown-linux-gnu" => "linux-ppc",
            "powerpc64-unknown-freebsd" => "BSD-generic64",
            "powerpc64-unknown-linux-gnu" => "linux-ppc64",
            "powerpc64-unknown-linux-musl" => "linux-ppc64",
            "powerpc64le-unknown-freebsd" => "BSD-generic64",
            "powerpc64le-unknown-linux-gnu" => "linux-ppc64le",
            "powerpc64le-unknown-linux-musl" => "linux-ppc64le",
            "riscv64gc-unknown-linux-gnu" => "linux-generic64",
            "s390x-unknown-linux-gnu" => "linux64-s390x",
            "s390x-unknown-linux-musl" => "linux64-s390x",
            "x86_64-apple-darwin" => "darwin64-x86_64-cc",
            "x86_64-unknown-freebsd" => "BSD-x86_64",
            "x86_64-unknown-dragonfly" => "BSD-x86_64",
            "x86_64-unknown-illumos" => "solaris64-x86_64-gcc",
            "x86_64-unknown-linux-gnu" => "linux-x86_64",
            "x86_64-unknown-linux-musl" => "linux-x86_64",
            "x86_64-unknown-openbsd" => "BSD-x86_64",
            "x86_64-unknown-netbsd" => "BSD-x86_64",
            "x86_64-sun-solaris" => "solaris64-x86_64-gcc",
            _ => panic!("don't know how to configure OpenSSL for {}", target),
        };
        configure.arg(os);

        let mut cc = cc::Build::new();
        cc.target(target).host(host).warnings(false).opt_level(2);
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
        if path.ends_with("-gcc") && !target.contains("unknown-linux-musl") {
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
            // For whatever reason `-static` on MUSL seems to cause
            // issues...
            if target.contains("musl") && arg == "-static" {
                continue;
            }

            // cc includes an `-arch` flag for Apple platforms, but we've
            // already selected an arch implicitly via the target above, and
            // OpenSSL contains about the conflict if both are specified.
            if target.contains("apple") {
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

        if target.contains("musl") {
            // Hack around openssl/openssl#7207 for now
            configure.arg("-DOPENSSL_NO_SECURE_MEMORY");
        }
        configure.current_dir(&inner_dir);
        self.run_command(configure, "configuring TASSL build");

        let mut depend = self.cmd_make();
        depend.arg("depend").current_dir(&inner_dir);
        self.run_command(depend, "building TASSL dependencies");

        let mut build = self.cmd_make();
        build.arg("build_libs").current_dir(&inner_dir);
        if let Some(s) = env::var_os("CARGO_MAKEFLAGS") {
            build.env("MAKEFLAGS", s);
        }
        self.run_command(build, "building TASSL");

        let mut install = self.cmd_make();
        install.arg("install").current_dir(&inner_dir);
        self.run_command(install, "installing TASSL");

        fs::remove_dir_all(&inner_dir).unwrap();

        Artifacts {
            lib_dir: install_dir.join("lib"),
            bin_dir: install_dir.join("bin"),
            include_dir: install_dir.join("include"),
            libs: vec!["ssl".to_string(), "crypto".to_string()],
            target: target.to_string(),
        }
    }

    fn run_command(&self, mut command: Command, desc: &str) {
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


impl Artifacts {
    pub fn include_dir(&self) -> &Path {
        &self.include_dir
    }

    pub fn lib_dir(&self) -> &Path {
        &self.lib_dir
    }

    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    pub fn libs(&self) -> &[String] {
        &self.libs
    }

    pub fn print_cargo_metadata(&self) {
        println!("cargo:rustc-link-search=native={}", self.lib_dir.display());
        for lib in self.libs.iter() {
            println!("cargo:rustc-link-lib=static={}", lib);
        }
        println!("cargo:include={}", self.include_dir.display());
        println!("cargo:lib={}", self.lib_dir.display());
    }
}
