use std::collections::HashMap;

#[cfg(test)]
mod test;

pub type Pos = usize;
pub type Line = usize;
pub type ExtCallback = fn(
    lines: &Vec<Vec<char>>,
    line: Line,
    pos: Pos,
    output: String,
    ctx: &Context,
) -> Result<(Line, Pos, String)>;

pub struct Context {
    pub plugins: HashMap<String, ExtCallback>,
    pub head_insert: String,
    pub body_insert: String,
}

impl Context {
    pub fn create(head_insert: String, body_insert: String) -> Self {
        Context {
            plugins: HashMap::new(),
            head_insert,
            body_insert,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    UnexpectedEnd(String),
    BlockNameNoEnd(Line, Pos),
    BlockNoEnd(Line, Pos),
    BadBlockName(Line, Pos),
    UnclosedBold(Line, Pos),
    UnclosedItalic(Line, Pos),
    UnclosedUnderline(Line, Pos),
    UnclosedStrikethrough(Line, Pos),
    StrayBackslash(Line, Pos),
    RecursiveList(Line, Pos),
    InvalidListDepth(Line, Pos),
    OtherError(Line, Pos, &'static str),
}

impl Error {
    pub fn message(&self) -> &'static str {
        match self {
            Self::UnexpectedEnd(_) => "This error is used internally.",
            Self::BlockNameNoEnd(_, _) => {
                "Block's name is not defined correctly as `[my_name ...]`."
            }
            Self::BlockNoEnd(_, _) => {
                "Block's openning `[` is not matched with a corresponding `]`."
            }
            Self::BadBlockName(_, _) => {
                "Block names must only use characters 0-9, a-z, A-Z, or '_'."
            }
            Self::UnclosedBold(_, _) => {
                "Openning `*` must be matched with a closing `*`. \
                                         Or, you meant to escape the `*` with `\\*`."
            }
            Self::UnclosedItalic(_, _) => {
                "Openning `/` must be matched with a closing `/`. \
                Or, you meant to escape the `/` with `\\/`."
            }
            Self::UnclosedUnderline(_, _) => {
                "Openning `_` must be matched with a closing `_`. \
                Or, you meant to escape the `_` with `\\_`."
            }
            Self::UnclosedStrikethrough(_, _) => {
                "Openning `~~` must be matched with a closing `~~`. \
                Or, you meant to escape the `~` with `\\~~`."
            }
            Self::StrayBackslash(_, _) => {
                "A stray `\\` is not allowed. \
                However, you can escape it using `\\\\`."
            }
            Self::RecursiveList(_, _) => {
                "Lists cannot Recurse. \
                In other words, you cannot do this: \
                `+ + Hello World`. \
                Perhaps you meant to use `++ Hello World`"
            }
            Self::InvalidListDepth(_, _) => {
                "List nesting depth is invalid. In other words: \
                `+ Layer One` cannot be followed by `++++ Layer Four!`."
            }
            Self::OtherError(_, _, error) => error,
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

pub fn parse(s: String, ctx: Context) -> Result<String> {
    //  <Boring HTML Stuff>
    let mut output = format!(
        "<html><head>{}</head><body>{}",
        ctx.head_insert, ctx.body_insert
    );
    let lines: Vec<Vec<char>> = s.split('\n').map(|s| s.chars().collect()).collect();
    let mut line = 0;
    let mut pos = 0;
    loop {
        //  jmp block_start     ; Find the block start character '['
        for vline in lines.iter().skip(line) {
            if let Some(npos) = vline.iter().skip(pos).position(|&c| c == '[') {
                pos = npos + pos + 1;
                break;
            }
            line += 1;
            pos = 0;
        }

        //  The fun part: parsing the block!
        match parse_block(&lines, line, pos, output, &ctx) {
            Ok((nline, npos, noutput)) => {
                line = nline;
                pos = npos;
                output = noutput;
            }
            Err(Error::UnexpectedEnd(noutput)) => {
                output = noutput;
                break;
            }
            e @ Err(_) => {
                e?;
                unreachable!("`e` always throws.")
            }
        }
    }

    //  </Boring HTML Stuff>
    output = output + "</body></html>";
    Ok(output)
}

const fn is_whitespace(c: char) -> bool {
    c == '\n' || c == '\t' || c == '\r' || c == ' '
}

const fn is_valid_ch(c: char) -> bool {
    let c = c as u8;
    (c >= b'A' && c <= b'Z') || (c >= b'a' && c <= b'z') || c == b'_'
}

//  Expects character after '['
pub fn parse_block(
    lines: &Vec<Vec<char>>,
    mut line: Line,
    mut pos: Pos,
    mut output: String,
    ctx: &Context,
) -> Result<(Line, Pos, String)> {
    let vline = lines
        .get(line)
        .ok_or(Error::UnexpectedEnd(output.clone()))?;

    //  Nice to meet you what's your name?
    let mut name = String::new();
    let mut no_name_end = Err(Error::BlockNameNoEnd(line, pos));
    let line_len = vline.len();
    for &(mut c) in vline.iter().skip(pos) {
        pos += 1;
        if line_len == pos && !is_whitespace(c) {
            name.push(c);
            c = ' ';
        }
        if is_whitespace(c) {
            no_name_end = Ok(());
            break;
        }
        if !is_valid_ch(c) {
            Err(Error::BadBlockName(line, pos))?
        }
        name.push(c);
    }
    no_name_end?;

    //  Look for a plugin to do the job or fall back to text parsing.
    if let Some(f) = ctx.plugins.get(&name) {
        f(lines, line, pos, output, ctx)
    } else {
        output = format!("{}<div class='{}'>", output, name);
        let mut last_list_was_ordered = None;
        loop {
            let (done, nline, npos, noutput, nlast_list_was_ordered) =
                parse_text_line(lines, line, pos, output, ctx, true, last_list_was_ordered)?;
            line = nline;
            pos = npos;
            output = noutput;
            last_list_was_ordered = nlast_list_was_ordered;
            if done {
                break;
            }
        }
        output = output + "</div>";
        Ok((line, pos, output))
    }
}

//  Additionally returns whether a genuine ']' was found.
pub fn parse_text_line(
    lines: &Vec<Vec<char>>,
    mut line: Line,
    mut pos: Pos,
    mut output: String,
    ctx: &Context,
    allow_lists: bool,
    last_list_was_ordered: Option<bool>,
) -> Result<(bool, Line, Pos, String, Option<bool>)> {
    if let Some(is_ordered) = last_list_was_ordered {
        output = parse_close_list(lines, line, pos, output, is_ordered);
    }
    let vline = lines.get(line).ok_or(Error::BlockNoEnd(line, pos))?;
    let mut bold = None;
    let mut italic = None;
    let mut underline = None;
    let mut strikethrough = None;
    let maybe_set = |var: &mut Option<(Line, Pos)>, output: &mut String, line, pos, c| {
        if var.is_some() {
            *output = format!("{}</{}>", output, c);
            *var = None
        } else {
            *output = format!("{}<{}>", output, c);
            *var = Some((line, pos))
        }
    };
    let mut line_first_valid_ch = true;
    //  (last character, last last character);
    let mut double_last_c = (' ', ' ');
    while let Some(&c) = vline.get(pos) {
        match c {
            '[' if double_last_c.0 != '\\' => {
                let (nline, npos, noutput) = parse_block(lines, line, pos + 1, output, ctx)?;
                line = nline;
                pos = npos;
                output = noutput;
                continue;
            }
            ']' if double_last_c.0 != '\\' => {
                if let Some((line, pos)) = bold {
                    Err(Error::UnclosedBold(line, pos))?
                }
                if let Some((line, pos)) = italic {
                    Err(Error::UnclosedItalic(line, pos))?
                }
                if let Some((line, pos)) = underline {
                    Err(Error::UnclosedUnderline(line, pos))?
                }
                if let Some((line, pos)) = strikethrough {
                    Err(Error::UnclosedStrikethrough(line, pos))?
                }
                return Ok((true, line, pos, output, None));
            }
            '+' if line_first_valid_ch => {
                if !allow_lists {
                    Err(Error::RecursiveList(line, pos))?
                }
                return parse_open_list(lines, line, pos, output, ctx, false);
            }
            '=' if line_first_valid_ch => {
                if !allow_lists {
                    Err(Error::RecursiveList(line, pos))?
                }
                return parse_open_list(lines, line, pos, output, ctx, true);
            }
            '*' if double_last_c.0 != '\\' => maybe_set(&mut bold, &mut output, line, pos, 'b'),
            '/' if double_last_c.0 != '\\' => maybe_set(&mut italic, &mut output, line, pos, 'i'),
            '_' if double_last_c.0 != '\\' => {
                maybe_set(&mut underline, &mut output, line, pos, 'u')
            }
            '~' if double_last_c.0 == '~' && double_last_c.1 != '\\' => {
                maybe_set(&mut strikethrough, &mut output, line, pos, 's')
            }
            '\\' if double_last_c.0 == '\\' => output.push(c),
            '\\' => {}
            ' ' if !is_whitespace(double_last_c.0) => output.push(c),
            ' ' => {}
            _ if ['*', '/', '_'].contains(&c) && double_last_c.0 == '\\' => {
                Err(Error::StrayBackslash(line, pos))?
            }
            _ if c != '~' && double_last_c.0 != '~' && double_last_c.1 == '\\' => {
                Err(Error::StrayBackslash(line, pos))?
            }
            _ => {
                output.push(c);
            }
        }
        pos += 1;
        if c == '\t' {
            continue;
        }
        if !is_whitespace(c) {
            line_first_valid_ch = false;
        }
        double_last_c = (c, double_last_c.0);
    }
    pos = 0;
    line += 1;
    if vline.len() == 0 {
        output.push_str("<br><br>");
    } else if !is_whitespace(double_last_c.0) {
        output.push(' ');
    }
    return Ok((false, line, pos, output, None));
}

fn parse_list_determine_depth(
    lines: &Vec<Vec<char>>,
    line: Line,
    mut pos: Pos,
    is_ordered: bool,
) -> Option<(usize, Pos)> {
    let listc = if is_ordered { '=' } else { '+' };
    let vline = lines.get(line)?;
    let mut depth = 0;
    let mut start = false;
    while let Some(&c) = vline.get(pos) {
        pos += 1;
        //  Walk to the first occurance of `listc`.
        if c == listc {
            start = true;
        }
        if !start {
            continue;
        }

        //  Increment depth until you run out of `listc`.
        depth += 1;
        if c != listc {
            break;
        }
    }
    if depth == 0 {
        None
    } else {
        Some((depth, pos))
    }
}

pub fn parse_open_list(
    lines: &Vec<Vec<char>>,
    mut line: Line,
    mut pos: Pos,
    mut output: String,
    ctx: &Context,
    is_ordered: bool,
) -> Result<(bool, Line, Pos, String, Option<bool>)> {
    enum ListManipulation {
        None,
        Push,
        Pop,
    }

    let depth;
    let mut manipulation = ListManipulation::None;
    if let Some((ndepth, npos)) = parse_list_determine_depth(lines, line, pos, is_ordered) {
        depth = ndepth;
        pos = npos;
    } else {
        unreachable!("In this case, `parse_list` should not have been called.")
    }
    if let Some((last_depth, _)) = parse_list_determine_depth(lines, line - 1, 0, is_ordered) {
        if depth - 1 == last_depth {
            manipulation = ListManipulation::Push;
        } else if depth + 1 == last_depth {
            manipulation = ListManipulation::Pop;
        } else if depth != last_depth {
            Err(Error::InvalidListDepth(line, pos))?
        }
    } else {
        manipulation = ListManipulation::Push
    }

    match manipulation {
        ListManipulation::Push => {
            output.push_str(if is_ordered { "<ol>" } else { "<ul>" });
        }
        ListManipulation::Pop => {
            output.push_str(if is_ordered { "</ol>" } else { "</ul>" });
        }
        _ => {}
    }

    output.push_str("<li>");
    let (done, nline, npos, noutput, _) =
        parse_text_line(lines, line, pos - 1, output, ctx, false, None)?;
    line = nline;
    pos = npos;
    output = noutput;
    output.push_str("</li>");

    Ok((done, line, pos, output, Some(is_ordered)))
}

pub fn parse_close_list(
    lines: &Vec<Vec<char>>,
    line: Line,
    pos: Pos,
    mut output: String,
    is_ordered: bool,
) -> String {
    if let (None, Some((depth, _))) = (
        parse_list_determine_depth(lines, line, pos, is_ordered),
        parse_list_determine_depth(lines, line - 1, 0, is_ordered),
    ) {
        for _ in 0..depth - 1 {
            output = output + if is_ordered { "</ol>" } else { "</ul>" };
        }
    }
    output
}
