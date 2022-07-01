use colored::Colorize;
use std::fs;
use std::io;

mod make;

fn help_and_exit() -> ! {
    eprintln!(
        r"
The {}'s Build Tool

Usage:
    osmlmk [options] <command> [project]

Options:
    -l | --lame     For Lame people who don't like color.
    -d | --dryrun   Don't actually write to output.
    -h | --help     Secretly does nothing.
    -f | --asdfjkl  Same as the previous flag.

Commands:
    i | init        Create a brand new project.
    b | build       Compile everything.
    c | clean       Clean up the mess I made.
    l | live        (WIP) Run a server to live reload this project.
",
        "Optimally Stupid Markup Language".blue().bold(),
    );
    std::process::exit(1);
}

#[cfg(windows)]
fn color_setup() {
    colored::control::set_virtual_terminal(true);
}

#[cfg(not(windows))]
fn color_setup() {}

fn main() {
    color_setup();
    let args = std::env::args().skip(1).collect();
    run(&cli(args));
}

#[derive(Debug, Clone, Copy)]
pub enum RunCommand {
    Init,
    Build,
    Clean,
    Live,
}

impl std::string::ToString for RunCommand {
    fn to_string(&self) -> String {
        match self {
            Self::Init => "init",
            Self::Build => "build",
            Self::Clean => "clean",
            Self::Live => "live",
        }
        .to_string()
    }
}

pub struct RunContext {
    success: bool,
    lame: bool,
    dryrun: bool,
    project_dir: String,
    command: RunCommand,
}

impl Drop for RunContext {
    fn drop(&mut self) {
        if self.success {
        } else {
        }
    }
}

fn cli(args: Vec<String>) -> RunContext {
    let mut ctx = RunContext {
        success: false,
        lame: false,
        dryrun: false,
        project_dir: String::new(),
        command: RunCommand::Init,
    };
    let mut commands = Vec::new();
    let mut project_dirs = Vec::new();
    for arg in args {
        match arg.as_str() {
            "i" | "init" => commands.push(RunCommand::Init),
            "b" | "build" => commands.push(RunCommand::Build),
            "c" | "clean" => commands.push(RunCommand::Clean),
            "l" | "live" => commands.push(RunCommand::Live),
            "-l" | "--lame" => ctx.lame = true,
            "-d" | "--dryrun" => ctx.dryrun = true,
            _ => project_dirs.push(arg),
        }
    }

    if commands.is_empty() {
        eprintln!("{} No commands given", "Make Error:".red().bold());
        help_and_exit();
    } else if commands.len() != 1 {
        eprint!(
            "{} Multiple commands given, including: ",
            "Make Error:".red().bold()
        );
        commands
            .iter()
            .for_each(|i| eprint!("`{}` ", i.to_string().yellow()));
        eprint!(".\n");
        help_and_exit();
    }

    if project_dirs.is_empty() {
        project_dirs.push("./".to_string())
    } else if project_dirs.len() != 1 {
        eprint!(
            "{} Multiple projects given, including: ",
            "Make Error:".red().bold()
        );
        project_dirs
            .iter()
            .for_each(|i| eprint!("`{}` ", i.yellow()));
        eprint!(".\n");
        help_and_exit();
    }

    ctx.command = *commands.get(0).unwrap();
    ctx.project_dir = project_dirs.get(0).unwrap().clone();
    ctx
}

fn io_error(str: &str, err: io::Error) -> ! {
    eprintln!("{} {} {}", "Make Error:".red().bold(), str, err.to_string());
    std::process::exit(1);
}

//  From this point on, `project_dir` will always be unwrapped.
//  I personally don't know anyone who could run `sudo chown` and `osmlmk` so perfectly,
//  that `project_dir`'s permissions change at exactly the right time to cause a crash.
fn run(ctx: &RunContext) {
    if ctx.lame {
        colored::control::set_override(false);
    }

    std::env::set_current_dir(&ctx.project_dir).unwrap_or_else(|_| {
        fs::create_dir(&ctx.project_dir).unwrap_or_else(|e| {
            io_error(
                format!(
                    "Unable to create project folder at {}",
                    ctx.project_dir.blue()
                )
                .as_str(),
                e,
            )
        });
        cmd_init(&ctx.project_dir);
    });

    match ctx.command {
        RunCommand::Init => cmd_init(&ctx.project_dir),
        RunCommand::Clean => cmd_clean(&ctx.project_dir),
        RunCommand::Build => cmd_build(&ctx, &ctx.project_dir),
        _ => unimplemented!(),
    }

            eprintln!(
                "\n{} Successfully ran `{}` at {}",
                "OK:".green().bold(),
                ctx.command.to_string().yellow().bold(),
                ctx.project_dir.blue()
            );
}

fn cmd_init(pdir: &String) {
    let creates = ["src/", "static/", "dist/", "dist/static/"];
    for create in creates {
        fs::create_dir(create).unwrap_or_else(|e| {
            if e.kind() != io::ErrorKind::AlreadyExists {
                io_error(
                    format!(
                        "Failed to create folder `{}` at `{}`",
                        create.blue(),
                        pdir.blue()
                    )
                    .as_str(),
                    e,
                )
            }
        });
    }
}

fn cmd_build(run_ctx: &RunContext, pdir: &String) {
    let mut build_ctx = make::load_build().unwrap_or_else(|e| {
        io_error(
            format!(
                "Failed to load build on `{}` at `{}`",
                "src/".blue(),
                pdir.blue()
            )
            .as_str(),
            e,
        );
    });
    make::execute_build(&run_ctx, &mut build_ctx).unwrap_or_else(|e| {
        io_error(
            format!(
                "Failed to execute build on `{}` at `{}`",
                "src/".blue(),
                pdir.blue()
            )
            .as_str(),
            e,
        );
    });
}

fn cmd_clean(pdir: &String) {
    enum FileType {
        Dir,
        File,
    } //  Let's be organized
    let cleans = [(FileType::Dir, "dist/"), (FileType::File, "osml.cache")];
    let mut errors = Vec::new();
    cleans.into_iter().for_each(|(ct, c)| match ct {
        FileType::Dir => {
            fs::remove_dir_all(c).unwrap_or_else(|e| {
                errors.push((c, e));
            });
        }
        FileType::File => {
            fs::remove_file(c).unwrap_or_else(|e| {
                errors.push((c, e));
            });
        }
    });
    let errors: Vec<(&str, io::Error)> = errors
        .into_iter()
        .filter(|(_, e)| e.kind() != io::ErrorKind::NotFound)
        .collect();
    errors.iter().for_each(|(name, e)| {
        eprintln!(
            "{} Could not remove `{}` in `{}` {}",
            "Make Error:".red().bold(),
            name.blue(),
            pdir.blue(),
            e
        );
    });
    if !errors.is_empty() {
        std::process::exit(1);
    }
    //  Laziness.
    cmd_init(pdir);
}
