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
}

impl Context {
    pub fn create() -> Self {
        Context {
            plugins: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Location {
    Null,
    Absolute(Line),
}

#[derive(Debug, Clone)]
pub struct Error {
    pub location: Location,
    pub error: ErrorType,
}

impl Error {
    pub fn null(et: ErrorType) -> Self {
        Error {
            location: Location::Null,
            error: et,
        }
    }

    pub fn abs(line: Line, et: ErrorType) -> Self {
        Error {
            location: Location::Absolute(line),
            error: et,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorType {
    UnexpectedEnd(String),
    BlockNameNoEnd, //  Impossible Error?
    ExpectedBlockStart,
    BlockNoEnd,
    BadBlockName,
    UnclosedBold,
    UnclosedItalic,
    UnclosedUnderline,
    UnclosedStrikethrough,
    StrayBackslash,
    RecursiveList,
    InvalidListDepth,
    OtherError(String),
}

impl ErrorType {
    pub fn message(&self) -> String {
        match self {
            Self::UnexpectedEnd(_) => "This error is used internally.",
            Self::ExpectedBlockStart => "Text cannot be placed outside of block: `[ ... ]`.",
            Self::BlockNameNoEnd => "Block's name is not defined correctly as `[my_name ...]`.",
            Self::BlockNoEnd => "Block's opening `[` is not matched with a corresponding `]`.",
            Self::BadBlockName => "Block names must only use characters 0-9, a-z, A-Z, or '_'.",
            Self::UnclosedBold => {
                "Opening `*` must be matched with a closing `*`. \
                                         Or, you meant to escape the `*` with `\\*`."
            }
            Self::UnclosedItalic => {
                "Opening `/` must be matched with a closing `/`. \
                Or, you meant to escape the `/` with `\\/`."
            }
            Self::UnclosedUnderline => {
                "Opening `_` must be matched with a closing `_`. \
                Or, you meant to escape the `_` with `\\_`."
            }
            Self::UnclosedStrikethrough => {
                "Opening `~` must be matched with a closing `~`. \
                Or, you meant to escape the `~` with `\\~`."
            }
            Self::StrayBackslash => {
                "A stray `\\` is not allowed. \
                However, you can escape it using `\\\\`."
            }
            Self::RecursiveList => {
                "Lists cannot recur. \
                In other words, you cannot do this: \
                `+ + Hello World`. \
                Perhaps you meant to use `++ Hello World`"
            }
            Self::InvalidListDepth => {
                "List nesting depth is invalid. In other words: \
                `+ Layer One` cannot be followed by `++++ Layer Four!`."
            }
            Self::OtherError(error) => error.as_str(),
        }
        .to_string()
    }
}

type Result<T> = std::result::Result<T, Error>;

pub fn parse(s: String, ctx: Context) -> Result<String> {
    //  <Boring HTML Stuff>
    let mut output = "<html><head></head><body>".to_string();
    let lines: Vec<Vec<char>> = s.split('\n').map(|s| s.chars().collect()).collect();
    let mut line = 0;
    let mut pos = 0;
    loop {
        //  jmp block_start     ; Find the block start character '['
        for vline in lines.iter().skip(line) {
            let mut found = false;
            let start_pos = pos;
            for idx in start_pos..vline.len() {
                let &c = vline.get(idx).unwrap();
                pos += 1;
                if c == '[' {
                    found = true;
                    break;
                }
                if !is_whitespace(c) {
                    Err(Error::abs(line, ErrorType::ExpectedBlockStart))?;
                }
            }
            if found {
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
            Err(Error {
                error: ErrorType::UnexpectedEnd(noutput),
                ..
            }) => {
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
        .ok_or(Error::null(ErrorType::UnexpectedEnd(output.clone())))?;

    //  Nice to meet you what's your name?
    let name_start_line = line;
    let mut name = String::new();
    let mut no_name_end = Err(Error::abs(line, ErrorType::BlockNameNoEnd));
    let line_len = vline.len();
    for &(mut c) in vline.iter().skip(pos) {
        //  Hack to get `[section]` to compile.
        if c == ']' {
            no_name_end = Ok(());
            break;
        }
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
            Err(Error::abs(name_start_line, ErrorType::BadBlockName))?
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
        let start_line = line;
        loop {
            let (done, nline, npos, noutput, nlast_list_was_ordered) = parse_text_line(
                lines,
                line,
                pos,
                output,
                ctx,
                true,
                start_line,
                last_list_was_ordered,
            )?;
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
    start_line: Line,
    last_list_was_ordered: Option<bool>,
) -> Result<(bool, Line, Pos, String, Option<bool>)> {
    if let Some(is_ordered) = last_list_was_ordered {
        output = parse_close_list(lines, line, pos, output, is_ordered);
    }
    let vline = lines
        .get(line)
        .ok_or(Error::abs(start_line, ErrorType::BlockNoEnd))?;
    let mut bold = None;
    let mut italic = None;
    let mut underline = None;
    let mut strikethrough = None;
    let check_format = |&bold, &italic, &underline, &strikethrough| -> Result<()> {
        if let &Some(line) = &bold {
            Err(Error::abs(line, ErrorType::UnclosedBold))?
        }
        if let &Some(line) = &italic {
            Err(Error::abs(line, ErrorType::UnclosedItalic))?
        }
        if let &Some(line) = &underline {
            Err(Error::abs(line, ErrorType::UnclosedUnderline))?
        }
        if let &Some(line) = &strikethrough {
            Err(Error::abs(line, ErrorType::UnclosedStrikethrough))?
        }
        Ok(())
    };
    let maybe_set = |var: &mut Option<Line>, output: &mut String, line, c| {
        if var.is_some() {
            *output = format!("{}</{}>", output, c);
            *var = None
        } else {
            *output = format!("{}<{}>", output, c);
            *var = Some(line)
        }
    };
    let mut line_first_valid_ch = true;
    //  (last character, last last character);
    let mut triple_last_c = (' ', ' ', ' ');
    while let Some(&c) = vline.get(pos) {
        match c {
            '[' if triple_last_c.0 != '\\' => {
                let (nline, npos, noutput) = parse_block(lines, line, pos + 1, output, ctx)?;
                line = nline;
                pos = npos;
                output = noutput;
                continue;
            }
            ']' if triple_last_c.0 != '\\' => {
                check_format(&bold, &italic, &underline, &strikethrough)?;
                return Ok((true, line, pos+1, output, None));
            }
            '+' if line_first_valid_ch => {
                if !allow_lists {
                    Err(Error::abs(line, ErrorType::RecursiveList))?
                }
                return parse_open_list(lines, line, pos, output, ctx, false, start_line);
            }
            '=' if line_first_valid_ch => {
                if !allow_lists {
                    Err(Error::abs(line, ErrorType::RecursiveList))?
                }
                return parse_open_list(lines, line, pos, output, ctx, true, start_line);
            }
            '*' if triple_last_c.0 != '\\'
                || (triple_last_c.0 == '\\' && triple_last_c.1 == '\\') =>
            {
                maybe_set(&mut bold, &mut output, line, 'b')
            }
            '/' if triple_last_c.0 != '\\'
                || (triple_last_c.0 == '\\' && triple_last_c.1 == '\\') =>
            {
                maybe_set(&mut italic, &mut output, line, 'i')
            }
            '_' if triple_last_c.0 != '\\'
                || (triple_last_c.0 == '\\' && triple_last_c.1 == '\\') =>
            {
                maybe_set(&mut underline, &mut output, line, 'u')
            }
            '~' if triple_last_c.0 != '\\'
                || (triple_last_c.0 == '\\' && triple_last_c.1 == '\\') =>
            {
                maybe_set(&mut strikethrough, &mut output, line, 's')
            }
            '\\' if triple_last_c.0 == '\\' => output.push(c),
            '\\' => {}
            ' ' if !is_whitespace(triple_last_c.0) => output.push(c),
            ' ' => {}
            _ if !['*', '/', '_', '~'].contains(&c) && triple_last_c.0 == '\\' => {
                Err(Error::abs(line, ErrorType::StrayBackslash))?
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
        triple_last_c = (c, triple_last_c.0, triple_last_c.1);
    }
    pos = 0;
    line += 1;
    if vline.len() == 0 {
        output.push_str("<br><br>");
    } else if !is_whitespace(triple_last_c.0) {
        output.push(' ');
    }
    check_format(&bold, &italic, &underline, &strikethrough)?;
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
    start_line: Line,
) -> Result<(bool, Line, Pos, String, Option<bool>)> {
    enum ListManipulation {
        Push,
        Pop,
    }

    let depth;
    let mut manipulations = vec![];
    if let Some((ndepth, npos)) = parse_list_determine_depth(lines, line, pos, is_ordered) {
        depth = ndepth;
        pos = npos;
    } else {
        unreachable!("In this case, `parse_list` should not have been called.")
    }
    if let Some((last_depth, _)) = parse_list_determine_depth(lines, line - 1, 0, is_ordered) {
        if depth - 1 == last_depth {
            manipulations.push(ListManipulation::Push);
        } else if depth < last_depth {
            (0..last_depth - depth)
                .into_iter()
                .for_each(|_| manipulations.push(ListManipulation::Pop));
        } else if depth != last_depth {
            Err(Error::abs(line, ErrorType::InvalidListDepth))?
        }
    } else {
        (0..depth - 1)
            .into_iter()
            .for_each(|_| manipulations.push(ListManipulation::Push));
    }

    for manipulation in manipulations {
        match manipulation {
            ListManipulation::Push => {
                output.push_str(if is_ordered { "<ol>" } else { "<ul>" });
            }
            ListManipulation::Pop => {
                output.push_str(if is_ordered { "</ol>" } else { "</ul>" });
            }
        }
    }

    output.push_str("<li>");
    let (done, nline, npos, noutput, _) = parse_text_line(
        lines,
        line,
        pos - 1,
        output,
        ctx,
        false,
        start_line,
        None,
    )?;
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
