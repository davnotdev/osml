use super::RunContext;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

const CONFIG_NAME: &'static str = "osml.ron";
const CACHE_NAME: &'static str = "osml.cache";

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
                        PathBuf::new()
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
                .for_each(|(f, e)| eprint!("\n\t`{}`: \"{}\"", f.blue(), e));
            eprint!("\n");
            std::process::exit(1);
        }
        BuildConfig { excluded }
    }
}

//  Holds canonicalized path names.
//  Includes both src/ and static/ files
struct BuildConfig {
    excluded: Vec<String>,
}

//  Source file names are stripped of .osml and relative to src/.
//  Statics are stored normally.
#[derive(Serialize, Deserialize)]
struct BuildCache {
    sources: HashMap<String, SystemTime>,
}

impl Drop for BuildCache {
    //  May write to src/ if drop is called in panic while . is set to src/.
    fn drop(&mut self) {
        let _ = fs::write(CACHE_NAME, ron::to_string(self).unwrap());
    }
}

pub fn check_create_file(file: &String) {
    let splits: Vec<&str> = file.split('/').collect();
    let name = splits.get(splits.len() - 1).unwrap();
    let mut dir = file.clone();
    (0..name.len()).into_iter().for_each(|_| {
        dir.pop();
    });
    let _ = fs::create_dir_all(dir);
    let _ = fs::read(file);
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
    let cache = BuildCache {
        sources: HashMap::new(),
    };
    let s = ron::to_string(&cache).unwrap();
    fs::write(CACHE_NAME, &s)?;
    Ok((cache, s))
}

pub fn execute_build(run_ctx: &RunContext, build_ctx: &mut BuildContext) -> io::Result<()> {
    let sources = list_sources()?;
    for source in sources {
        if let Some((name, time)) = compile_source(run_ctx, build_ctx, &source) {
            build_ctx.cache.sources.insert(name, time);
        }
    }
    let statics = list_statics()?;
    let remove_statics = list_remove_statics(&statics)?;
    for remove_static in remove_statics.iter() {
        let mut cont = false;
        fs::remove_file(remove_static).unwrap_or_else(|e| {
            eprintln!(
                "{} Unable to access `{}` {}",
                "\tError:".red().bold(),
                remove_static.blue(),
                e
            );
            cont = true;
        });
        if cont {
            continue;
        }
        eprintln!(
            "{} {} --> {}",
            "\tOK:".green().bold(),
            remove_static.bold(),
            "/dev/null".bold()
        )
    }
    for static_src in statics {
        compile_static(&static_src);
    }
    Ok(())
}

fn list_sources() -> io::Result<Vec<String>> {
    std::env::set_current_dir("src/")?;
    let res = recurse_walk_dir(".").map(|sources| {
        sources
            .into_iter()
            .filter_map(|mut path| {
                if let Some("osml") = path.extension().map(|p| p.to_str().unwrap()) {
                    path.set_extension("");
                    let mut path = path.to_str().unwrap().to_string();
                    //  Try to remove the `./` in front bc it's ugly.
                    path.remove(0);
                    path.remove(0);
                    return Some(path);
                }
                None
            })
            .collect()
    });
    std::env::set_current_dir("..")?;
    res
}

fn list_statics() -> io::Result<Vec<String>> {
    list_statics_anywhere("./static/")
}

fn list_remove_statics(statics_list: &Vec<String>) -> io::Result<Vec<String>> {
    let built = list_statics_anywhere("./dist/static/")?;
    Ok(built
        .into_iter()
        .map(|mut file| {
            (0..=4).into_iter().for_each(|_| {
                file.remove(0);
            });
            file
        })
        .filter_map(|file| {
            if !statics_list.contains(&file) {
                return Some("dist/".to_string() + &file);
            }
            None
        })
        .collect())
}

fn list_statics_anywhere(location: &str) -> io::Result<Vec<String>> {
    let res = recurse_walk_dir(location).map(|statics| {
        statics
            .into_iter()
            .map(|path| {
                let mut path = path.to_str().unwrap().to_string();
                //  Try to remove the `./` in front bc it's ugly.
                path.remove(0);
                path.remove(0);
                path
            })
            .collect()
    });
    res
}

fn recurse_walk_dir(dir: &str) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in fs::read_dir(dir)? {
        let path = path?;
        if path.file_type()?.is_dir() {
            files.append(&mut recurse_walk_dir(path.path().to_str().unwrap())?);
        } else if path.file_type()?.is_file() {
            files.push(path.path());
        }
    }
    Ok(files)
}

fn compile_source(
    run_ctx: &RunContext,
    build_ctx: &BuildContext,
    src: &String,
) -> Option<(String, SystemTime)> {
    let should_compile_res = should_compile_source(build_ctx, src);
    if let Some(_) = should_compile_res {
        let mut cmd = std::process::Command::new("./osmlc");
        let src_name = ("src/".to_string() + src + ".osml").to_string();
        let dst_name = ("dist/".to_string() + src + ".html").to_string();
        check_create_file(&dst_name);
        cmd.args([src_name.as_str(), "-o", dst_name.as_str(), "-c"]);
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
                "\tError:".red().bold(),
                src_name.bold(),
                dst_name.bold(),
            );
            eprintln!("---------\n");
            for b in out.stderr {
                eprint!("{}", b as char)
            }
            eprintln!("---------\n");
            std::process::exit(1);
        } else {
            eprintln!(
                "{} {} --> {}",
                "\tOK:".green().bold(),
                src_name.bold(),
                dst_name.bold(),
            );
        }
    }
    should_compile_res
}

//  This doesn't need to be run if the file already exists.
fn compile_static(src: &String) {
    //  maybe move this out of looop.
    let _ = fs::create_dir("dist/static/");
    let dst_name = "dist/".to_string() + src;
    if should_compile_static(&dst_name) {
        check_create_file(&dst_name);
        fs::hard_link(src, &dst_name).unwrap_or_else(|e| {
            eprintln!("Failed to get `{}` {}", src.blue(), e);
            std::process::exit(1);
        });
        eprintln!(
            "{} {} --> {}",
            "\tOK:".green().bold(),
            src.bold(),
            dst_name.bold(),
        );
    }
}

fn should_compile_source(ctx: &BuildContext, src: &String) -> Option<(String, SystemTime)> {
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
    if let Some(last_modify) = ctx.cache.sources.get(src) {
        if last_modify == &modify {
            None?;
        }
    }
    Some((src.clone(), modify))
}

fn should_compile_static(src: &String) -> bool {
    fs::read_to_string(src).is_err()
}
