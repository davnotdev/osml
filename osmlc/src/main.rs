use colored::Colorize;
use libosml::{parse, Context};
use std::fs;

fn help_and_exit() -> ! {
    eprintln!(
        r"
The {}'s Compiler

Usage: 
    osmlc [options] <input> -o <output>

Options:
    -o              Specify your output.
    -c | --color    Forces color 24/7 100% of the time.
    -l | --lame     For Lame people who don't like color. *
    -d | --dryrun   Don't actually write to output.
    -h | --help     Secretly does nothing.
    -f | --asdfjkl  Same as the previous flag.

* You {} remove {}{}{}{}{}{} from this message. >:D
", //  Color = Cool
        "Optimally Stupid Markup Language".blue().bold(),
        "cannot".red().bold(),
        "c".truecolor(193, 177, 0).bold(),
        "o".truecolor(244, 139, 44).bold(),
        "l".truecolor(255, 100, 103).bold(),
        "o".truecolor(247, 87, 167).bold(),
        "u".truecolor(181, 107, 219).bold(),
        "r".truecolor(20, 129, 240).bold(),
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
    run(&cli(args))
}

#[derive(Debug)]
pub struct RunContext {
    color: Option<()>,
    lame: bool,
    dryrun: bool,
    input: String,
    output: String,
}

fn cli(args: Vec<String>) -> RunContext {
    let mut ctx = RunContext {
        color: None,
        lame: false,
        dryrun: false,
        input: String::new(),
        output: String::new(),
    };

    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    let mut was_o_flag = false;
    for arg in args.iter() {
        match arg.as_str() {
            "-l" | "--lame" => ctx.lame = true,
            "-c" | "--color" => ctx.color = Some(()),
            "-d" | "-dryrun" => ctx.dryrun = true,
            "-o" => was_o_flag = true,
            _ if was_o_flag => {
                outputs.push(arg.clone());
            }
            _ => {
                inputs.push(arg.clone());
            }
        }
    }

    let mut error = false;
    if inputs.len() != 1 {
        if inputs.is_empty() {
            eprintln!("{} No inputs given", "Error:".red().bold());
        } else {
            eprint!(
                "{} Multiple inputs given, including: ",
                "Error:".red().bold()
            );
            inputs.iter().for_each(|i| eprint!("`{}` ", i.yellow()));
            eprint!(".\n");
        }
        error = true;
    }

    if outputs.len() != 1 {
        if outputs.is_empty() {
            eprintln!("{} No outputs given", "Error:".red().bold());
        } else {
            eprint!(
                "{} Multiple outputs given, including: ",
                "Error:".red().bold()
            );
            outputs.iter().for_each(|i| eprint!("`{}` ", i.yellow()));
            eprint!(".\n");
        }
        error = true;
    }

    if error {
        help_and_exit();
    }

    ctx.input = inputs.get(0).unwrap().clone();
    ctx.output = outputs.get(0).unwrap().clone();

    ctx
}

fn run(ctx: &RunContext) {
    //  --lame should always have precedent over --color
    if let Some(_) = ctx.color {
        colored::control::set_override(true)
    }
    if ctx.lame {
        colored::control::set_override(false)
    }

    let input = fs::read_to_string(&ctx.input).unwrap_or_else(|e| {
        eprintln!(
            "{} Couldn't open input file: `{}`, {}",
            "Error:".red().bold(),
            ctx.input.yellow(),
            e
        );
        std::process::exit(1)
    });

    let parsed = parse(input, Context::create(String::new(), String::new())).unwrap_or_else(|e| {
        eprintln!("{} {:?}", "Error:".red().bold(), e);
        std::process::exit(1);
    });

    if !ctx.dryrun {
        fs::write(&ctx.output, parsed).unwrap_or_else(|e| {
            eprintln!(
                "{} Couldn't open output file: `{}`, {}",
                "Error:".red().bold(),
                ctx.output.yellow(),
                e
            );
            std::process::exit(1)
        });
    }
}
