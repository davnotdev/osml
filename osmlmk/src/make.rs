use super::RunContext;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::time::SystemTime;

const CONFIG_NAME: &'static str = "osml.ron";
const CACHE_NAME: &'static str = "osml.cache";

//  All stored file names are stripped of their 'osml' extension and are relative to src/.

pub struct BuildContext {
    cache: BuildCache,
    config: BuildConfig,
}

#[derive(Serialize, Deserialize)]
struct LoadBuildConfig {
    excluded: Vec<String>,
}

impl LoadBuildConfig {
    pub fn into_config(self) -> BuildConfig {
        let mut excluded = Vec::new();
        let mut errors = Vec::new();
        for exclude in self.excluded {
            excluded.push(
                fs::canonicalize(&exclude)
                    .unwrap_or_else(|e| {
                        errors.push((exclude, e));
                        std::path::PathBuf::new()
                    })
                    .to_str()
                    .unwrap()
                    .to_string(),
            );
        }
        if !errors.is_empty() {
            eprint!(
                "{} Could not open the following files in `osml.ron`: ",
                "Make Error:".red().bold()
            );
            errors
                .iter()
                .for_each(|(f, e)| eprint!("\n\t`{}`: \"{}\"", f.blue().bold(), e));
            eprint!("\n");
            std::process::exit(1);
        }
        BuildConfig { excluded }
    }
}

//  Holds canonicalized path names.
struct BuildConfig {
    excluded: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct BuildCache(HashMap<String, SystemTime>);

impl Drop for BuildCache {
    fn drop(&mut self) {
        let _ = fs::write(CACHE_NAME, ron::to_string(self).unwrap());
    }
}

pub fn load_build() -> io::Result<BuildContext> {
    //  Try to load cache and config.
    //  Creates new cache and config file if fails.
    let cache = fs::read_to_string(CACHE_NAME)
        .map(|s| Ok(s) as io::Result<String>)
        .unwrap_or_else(|_| Ok(clean_cache()?.1))?;
    let config = fs::read_to_string(CONFIG_NAME)
        .map(|s| Ok(s) as io::Result<String>)
        .unwrap_or_else(|_| {
            let s = ron::to_string(&LoadBuildConfig {
                excluded: Vec::new(),
            })
            .unwrap();
            fs::write(CONFIG_NAME, &s)?;
            Ok(s)
        })?;

    //  Parse and load cache and config.
    //  Creates new cache or throws on config fail.
    let cache = ron::from_str::<BuildCache>(cache.as_str())
        .map(|res| Ok(res) as io::Result<BuildCache>)
        .unwrap_or_else(|_| Ok(clean_cache()?.0))?;
    let config = ron::from_str::<LoadBuildConfig>(config.as_str())
        .unwrap_or_else(|e| {
            eprintln!(
                "{} Got error while parsing `osml.ron` {}",
                "Make Error:".red().bold(),
                e
            );
            std::process::exit(1);
        })
        .into_config();
    Ok(BuildContext { cache, config })
}

fn clean_cache() -> io::Result<(BuildCache, String)> {
    let cache = BuildCache(HashMap::new());
    let s = ron::to_string(&BuildCache(HashMap::new())).unwrap();
    fs::write(CACHE_NAME, &s)?;
    Ok((cache, s))
}

pub fn execute_build(run_ctx: &RunContext, build_ctx: &mut BuildContext) -> io::Result<()> {
    let sources = list_sources()?;
    for source in sources {
        if let Some((name, time)) = compile_source(run_ctx, build_ctx, &source) {
            build_ctx.cache.0.insert(name, time);
        }
    }
    build_static()?;
    Ok(())
}

//  `cp -rf ./static ./dist/static`
fn build_static() -> io::Result<()> {
    let _ = fs::remove_file("dist/static/");
    fs::copy("static/", "dist/static/")?;
    eprintln!("{} {} --> {}", "OK:".green().bold(), "static/".bold(), "dist/static/".bold());
    Ok(())
}

#[cfg(windows)]
fn link_dir(src: &str, dst: &str) -> io::Result<()> {
    //  See dwFlags of CreateSymbolicLinkW
    //  std::os::windows::fs::symlink(src, dst)
    build_static("static/", "dist/static/")
}

#[cfg(unix)]
fn link_dir(src: &str, dst: &str) -> io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

//  `ln -s ./static ./dist/static`
fn link_static(build_ctx: &BuildContext) -> io::Result<()> {
    if let Err(e) = link_dir("static/", "dist/static/") {
        if e.kind() != io::ErrorKind::AlreadyExists {
            Err(e)?
        }
    }
    Ok(())
}

fn list_sources() -> io::Result<Vec<String>> {
    std::env::set_current_dir("src/")?;
    let res = recurse_list_sources(&".".to_string(), 0);
    std::env::set_current_dir("..")?;
    res
}

fn recurse_list_sources(dir: &String, depth: usize) -> io::Result<Vec<String>> {
    //  Number arbitrarily picked for no specific reason.
    if depth >= 420 {
        eprintln!(
            "{} Folder `{}` infinitly loops back into itself!",
            "Make Error:".red().bold(),
            dir
        );
    }

    //  Visit and filter `osml` files.
    let mut sources = Vec::new();
    for path in fs::read_dir(dir)? {
        let path = path?;
        let path_name = path.path().to_str().unwrap().to_string();
        if let Ok(_) = fs::read_dir(path.path()) {
            sources.append(&mut recurse_list_sources(&path_name, depth + 1)?);
        } else {
            let mut path = path.path();
            if path.extension().unwrap() == "osml" {
                path.set_extension("");
                let mut path = path.to_str().unwrap().to_string();
                //  Try to remove the `./` in front bc its ugly.
                path.remove(0);
                path.remove(0);
                sources.push(path);
            }
        }
    }
    Ok(sources)
}

fn compile_source(
    run_ctx: &RunContext,
    build_ctx: &BuildContext,
    src: &String,
) -> Option<(String, SystemTime)> {
    let should_compile_res = should_compile(build_ctx, src);
    if let Some(_) = should_compile_res {
        let mut cmd = std::process::Command::new("./osmlc");
        let src_print_name = ("src/".to_string() + src + ".osml").to_string();
        let dst_print_name = ("dist/".to_string() + src + ".html").to_string();
        cmd.args([src_print_name.as_str(), "-o", dst_print_name.as_str(), "-c"]);
        if run_ctx.lame {
            cmd.arg("-l");
        }
        if run_ctx.dryrun {
            cmd.arg("-d");
        }
        let out = cmd.output();
        if let Err(_) = out {
            eprintln!("{} Could not execute osmlc", "Make Error:".red().bold());
            std::process::exit(1);
        }
        let out = out.unwrap();
        if !out.stderr.is_empty() {
            eprintln!(
                "{} {} --> {}",
                "Error".red().bold(),
                src_print_name.bold(),
                dst_print_name.bold(),
            );
            for b in out.stderr {
                eprint!("{}", b as char)
            }
            std::process::exit(1);
        } else {
            eprintln!(
                "{} {} --> {}",
                "OK:".green().bold(),
                src_print_name.bold(),
                dst_print_name.bold(),
            );
        }
    }
    should_compile_res
}

fn should_compile(ctx: &BuildContext, src: &String) -> Option<(String, SystemTime)> {
    let true_src = "src/".to_string() + src + ".osml";

    if ctx.config.excluded.contains(
        &fs::canonicalize(&true_src)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
    ) {
        None?
    }

    let metadata_res = fs::metadata(true_src);
    if let Err(ref e) = metadata_res {
        if e.kind() == io::ErrorKind::Unsupported {
            None?
        }
    }
    let modify = metadata_res.unwrap().modified().unwrap();
    if let Some(last_modify) = ctx.cache.0.get(src) {
        if last_modify == &modify {
            None?;
        }
    }
    Some((src.clone(), modify))
}
