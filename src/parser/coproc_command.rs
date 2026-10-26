use super::*;
use crate::lexer::{Token, TokenKind};

pub(super) fn parse_coproc_command(tokens: &[Token], start: usize) -> Option<(CommandNode, usize)> {
    // Parse `coproc [NAME] command [args...]` or `coproc [NAME] { body; }` or `coproc [NAME] ( body )`
    let mut i = start + 1; // skip `coproc`

    // Determine if next token is a name followed by a compound command,
    // or if the next token is itself the command.
    let mut name: Option<(String, String)> = None;
    let lookahead = tokens.get(i);
    if let Some(lookahead) = lookahead {
        let is_brace = is_brace_group_token(lookahead) || is_keyword(tokens, i, "{");
        let is_subshell = is_keyword(tokens, i, "(");
        let is_compound = is_brace || is_subshell || is_coproc_shell_command_start(tokens, i);

        if !is_compound {
            // This might be a name. Check if the token *after* it is a brace group or subshell.
            let next_after = tokens.get(i + 1);
            let next_is_compound = next_after.is_some_and(|t| is_brace_group_token(t))
                || is_keyword(tokens, i + 1, "{")
                || is_keyword(tokens, i + 1, "(")
                || is_coproc_shell_command_start(tokens, i + 1);
            // GNU parse.y:1125-1175 coproc grammar: `COPROC WORD
            // shell_command` — WORD is the coproc NAME only when what
            // follows is a compound/shell command. Before a SIMPLE
            // command there is no name production at all: `coproc CP
            // echo hi` is the unnamed form running the simple command
            // `CP echo hi` (the child reports `CP: command not found`
            // and the fds bind as COPROC). A builtin-name allowlist
            // here stole the first word of the simple command as a
            // name (rubash#1153).
            // rubash#1652: `!` is never a coproc NAME — when it sits
            // at the operand position the bang check below must see
            // it, so it is excluded from name consumption even when
            // a compound body follows (`coproc ! { ...; }` is GNU's
            // syntax error, not a coproc named `!`).
            if !lookahead.value.starts_with('-')
                && next_is_compound
                && !is_keyword(tokens, i, "!")
            {
                name = Some((lookahead.value.clone(), lookahead.raw.clone()));
                i += 1; // consume the name
            }
            // Otherwise: no name, the token is part of the simple command
        }
    }

    // GNU parse.y: the coproc operand is a shell_command or a
    // simple_command — neither can begin with the pipeline-level `!'
    // (BANG belongs to pipeline grammar, and a coproc is not a
    // pipeline), so `coproc ! ...` and `coproc NAME ! ...` are syntax
    // errors, not negated coprocs (rubash#1652). The `!' used to fall
    // into the simple-command word collection below and reach the
    // respawned child, which quietly executed it as a negation.
    if is_keyword(tokens, i, "!") {
        return Some((coproc_bang_error_node(tokens, start), i + 1));
    }

    // Parse the body
    if let Some(token) = tokens.get(i) {
        // Brace group body (single token from lexer)
        if is_brace_group_token(token) {
            let body = parse_inline_brace_body(token);
            let mut command = CommandNode::new();
            command.line = tokens.get(start).map(|t| t.position);
            command.logical_line = tokens.get(start).map(|t| t.logical_line);
            command.coproc_command = Some(coproc_command(
                name,
                Vec::new(),
                Vec::new(),
                CoprocBodyKind::BraceGroup,
                Some(("{".to_string(), "}".to_string())),
                Some(body),
            ));
            return Some(finish_coproc_command(command, tokens, i + 1));
        }

        if is_keyword(tokens, i, "{") {
            let (body, close_i) = parse_split_brace_body(tokens, i)?;
            let mut command = CommandNode::new();
            command.line = tokens.get(start).map(|t| t.position);
            command.logical_line = tokens.get(start).map(|t| t.logical_line);
            command.coproc_command = Some(coproc_command(
                name,
                Vec::new(),
                Vec::new(),
                CoprocBodyKind::BraceGroup,
                Some((tokens[i].value.clone(), tokens[close_i].value.clone())),
                Some(body),
            ));
            return Some(finish_coproc_command(command, tokens, close_i + 1));
        }

        if let Some((body_command, body_end)) = parse_coproc_compound_body(tokens, i) {
            let mut command = CommandNode::new();
            command.line = tokens.get(start).map(|t| t.position);
            command.logical_line = tokens.get(start).map(|t| t.logical_line);
            command.coproc_command = Some(coproc_command(
                name,
                Vec::new(),
                Vec::new(),
                CoprocBodyKind::CompoundCommand,
                None,
                Some(vec![body_command]),
            ));
            return Some(finish_coproc_command(command, tokens, body_end));
        }

        if let Some((body, body_end)) = parse_coproc_command_sequence_body(tokens, i) {
            let mut command = CommandNode::new();
            command.line = tokens.get(start).map(|t| t.position);
            command.logical_line = tokens.get(start).map(|t| t.logical_line);
            command.coproc_command = Some(coproc_command(
                name,
                Vec::new(),
                Vec::new(),
                CoprocBodyKind::CommandSequence,
                None,
                Some(body),
            ));
            return Some(finish_coproc_command(command, tokens, body_end));
        }

        // Subshell body: ( ... )
        if is_keyword(tokens, i, "(") {
            i += 1; // consume `(`
            let body_start = i;
            let mut depth = 1usize;
            let mut case_depth = 0usize;
            while i < tokens.len() {
                let boundary = i == body_start || command_boundary_keyword_allowed(tokens, i);
                if boundary && is_keyword(tokens, i, "case") {
                    case_depth += 1;
                } else if (boundary || follows_case_in_keyword(tokens, i))
                    && is_case_end_keyword(tokens, i)
                {
                    // `esac` right after the case's `in` closes the empty
                    // case (parse.y:3428-3441 special_case_tokens) so
                    // `coproc ( case x in esac )` ends at its own `)`
                    // (rubash#336).
                    case_depth = case_depth.saturating_sub(1);
                } else if case_depth == 0 && is_keyword(tokens, i, "(") {
                    depth += 1;
                } else if case_depth == 0 && is_keyword(tokens, i, ")") {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                i += 1;
            }
            if i < tokens.len() {
                let body = parse(&tokens[body_start..i]).commands;
                let mut command = CommandNode::new();
                command.line = tokens.get(start).map(|t| t.position);
                command.logical_line = tokens.get(start).map(|t| t.logical_line);
                command.coproc_command = Some(coproc_command(
                    name,
                    Vec::new(),
                    Vec::new(),
                    CoprocBodyKind::Subshell,
                    Some(("(".to_string(), tokens[i].value.clone())),
                    Some(body),
                ));
                return Some(finish_coproc_command(command, tokens, i + 1));
            }
        }
    }

    // Simple command: collect remaining tokens as words
    let mut words: Vec<String> = Vec::new();
    let mut word_metadata = Vec::new();
    while i < tokens.len() {
        // rubash#1652 (cont.): a standalone `!' directly after the
        // command-name word is the same grammar error as in operand
        // position — GNU's lexer still returns BANG there, and the
        // coproc productions have no slot for it (`coproc echo ! hi'
        // and `coproc NC ! echo hi' both exit 2 under GNU). Once an
        // assignment prefix or a second word intervenes, `!' is an
        // ordinary argument word (`coproc x=1 ! echo hi',
        // `coproc echo a ! hi' both run), so the check is exactly:
        // one collected word, and it is not an assignment.
        if is_keyword(tokens, i, "!")
            && words.len() == 1
            && !coproc_word_is_assignment(words[0].as_str())
        {
            return Some((coproc_bang_error_node(tokens, start), i + 1));
        }
        if let Some((word, next_i)) = collect_compound_or_keyword_word_value(tokens, i) {
            let raw = if next_i == i + 1 {
                tokens[i].raw.as_str()
            } else {
                word.as_str()
            };
            // rubash#1653: a leading `time` KEYWORD token is the simple
            // command's first word (see parse_coproc_compound_body), not
            // a prefix. The coproc body is respawned from the words'
            // source text, where a bare `time` would regain keyword
            // status and never report `time: command not found`; the
            // quoted raw keeps it a plain command name in the child.
            let raw = if words.is_empty()
                && tokens[i].kind == TokenKind::Keyword
                && tokens[i].value == "time"
            {
                "'time'"
            } else {
                raw
            };
            word_metadata.push(build_word_metadata(words.len(), &word, raw));
            words.push(word);
            i = next_i;
        } else {
            break;
        }
    }

    if words.is_empty() {
        return None;
    }

    let mut command = CommandNode::new();
    command.line = tokens.get(start).map(|t| t.position);
    command.logical_line = tokens.get(start).map(|t| t.logical_line);
    command.coproc_command = Some(coproc_command(
        name,
        words,
        word_metadata,
        CoprocBodyKind::SimpleCommand,
        None,
        None,
    ));
    Some(finish_coproc_command(command, tokens, i))
}

fn finish_coproc_command(
    command: CommandNode,
    tokens: &[Token],
    index: usize,
) -> (CommandNode, usize) {
    let (command, mut next_i) = finish_compound_command(command, tokens, index);
    while tokens
        .get(next_i)
        .is_some_and(|token| token.kind == TokenKind::Semicolon)
    {
        next_i += 1;
    }
    (command, next_i)
}

fn coproc_command(
    name: Option<(String, String)>,
    words: Vec<String>,
    word_metadata: Vec<WordMetadata>,
    body_kind: CoprocBodyKind,
    body_delimiters: Option<(String, String)>,
    body: Option<Vec<CommandNode>>,
) -> Box<CoprocCommand> {
    let (body_open_delimiter, body_close_delimiter) = body_delimiters
        .map(|(open, close)| (Some(open), Some(close)))
        .unwrap_or((None, None));
    let body_open_delimiter_metadata = body_open_delimiter
        .as_deref()
        .map(synthetic_delimiter_metadata);
    let body_close_delimiter_metadata = body_close_delimiter
        .as_deref()
        .map(synthetic_delimiter_metadata);
    let (name, name_metadata) = name
        .map(|(value, raw)| {
            let metadata = build_word_metadata(0, &value, &raw);
            (Some(value), Some(Box::new(metadata)))
        })
        .unwrap_or((None, None));
    Box::new(CoprocCommand {
        keyword: "coproc".to_string(),
        keyword_metadata: synthetic_delimiter_metadata("coproc"),
        name,
        name_metadata,
        words,
        word_metadata,
        body_kind,
        body_open_delimiter,
        body_open_delimiter_metadata,
        body_close_delimiter,
        body_close_delimiter_metadata,
        body,
    })
}

fn synthetic_delimiter_metadata(delimiter: &str) -> Box<WordMetadata> {
    Box::new(build_word_metadata(0, delimiter, delimiter))
}

/// The parse-error marker node for a `!' the coproc grammar cannot
/// place (rubash#1652); rendered by the executor as
/// `syntax error near unexpected token `!'' with exit status 2.
fn coproc_bang_error_node(tokens: &[Token], start: usize) -> CommandNode {
    let mut command = CommandNode::new();
    command.line = tokens.get(start).map(|t| t.position);
    command.logical_line = tokens.get(start).map(|t| t.logical_line);
    command.insert_assignment(
        "__RUBASH_PARSE_ERROR__".to_string(),
        "unexpected token `!'".to_string(),
    );
    command
}

/// GNU assignment-word shape at the head of a simple command: a name
/// (`[A-Za-z_][A-Za-z0-9_]*`, optionally with a `[...]` subscript)
/// followed by `=` or `+=`.
fn coproc_word_is_assignment(word: &str) -> bool {
    let bytes = word.as_bytes();
    let mut i = 0;
    if i >= bytes.len() || !(bytes[i] == b'_' || bytes[i].is_ascii_alphabetic()) {
        return false;
    }
    while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'[' {
        match word[i..].find(']') {
            Some(close) => i += close + 1,
            None => return false,
        }
    }
    if i < bytes.len() && bytes[i] == b'+' {
        i += 1;
    }
    i < bytes.len() && bytes[i] == b'='
}

/// Does a compound (shell_command) body start at `index`?
/// rubash#1653: `time` is deliberately NOT a start and has no
/// prefix form here — GNU's coproc productions (parse.y:1125-1173)
/// are only `COPROC [WORD] shell_command | simple_command`, and a
/// timed pipeline is neither, so `coproc time { ...; }` binds
/// `time` as the coproc NAME (the WORD production) and `coproc
/// time CMD` runs the simple command whose head word is `time`.
fn is_coproc_shell_command_start(tokens: &[Token], index: usize) -> bool {
    tokens.get(index).is_some_and(|token| {
        matches!(
            token.value.as_str(),
            "for" | "case" | "select" | "coproc" | "if" | "while" | "until" | "[["
        ) || token.value.starts_with("((")
            || parse_function_command(tokens, index).is_some()
    })
}

fn parse_coproc_command_sequence_body(
    tokens: &[Token],
    start: usize,
) -> Option<(Vec<CommandNode>, usize)> {
    let end = match tokens.get(start)?.value.as_str() {
        "[[" => matching_coproc_conditional_end(tokens, start)?,
        "if" => matching_coproc_if_end(tokens, start)?,
        "while" | "until" => matching_coproc_loop_end(tokens, start)?,
        _ => return None,
    };
    Some((parse(&tokens[start..=end]).commands, end + 1))
}

fn matching_coproc_conditional_end(tokens: &[Token], start: usize) -> Option<usize> {
    (start..tokens.len()).find(|&index| tokens[index].raw == "]]")
}

fn matching_coproc_if_end(tokens: &[Token], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in start..tokens.len() {
        let boundary = index == start || command_boundary_keyword_allowed(tokens, index);
        if boundary && is_keyword(tokens, index, "if") {
            depth += 1;
        } else if boundary && is_keyword(tokens, index, "fi") {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn matching_coproc_loop_end(tokens: &[Token], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in start..tokens.len() {
        let boundary = index == start || command_boundary_keyword_allowed(tokens, index);
        if boundary
            && (is_keyword(tokens, index, "for")
                || is_keyword(tokens, index, "while")
                || is_keyword(tokens, index, "until")
                || is_keyword(tokens, index, "select"))
        {
            depth += 1;
        } else if boundary && is_keyword(tokens, index, "done") {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn parse_coproc_compound_body(tokens: &[Token], start: usize) -> Option<(CommandNode, usize)> {
    if let Some(parsed) = parse_arithmetic_command(tokens, start) {
        return Some(parsed);
    }
    if let Some(parsed) = parse_function_command(tokens, start) {
        return Some(parsed);
    }

    match tokens.get(start)?.value.as_str() {
        // GNU parse.y coproc productions take a shell_command or a
        // simple_command — `time` is a pipeline-level prefix, never a
        // shell_command, so after `coproc` it can only be the first
        // word of a simple command (`coproc time echo hi` runs the
        // command named `time`, which does not exist — rubash#1653).
        // Routing it to the time-prefixed productions swallowed a
        // following pipeline into the coproc body and bound the fds in
        // the parent. The simple-command collection below takes it.
        "for" => parse_for_command(tokens, start, None, 0),
        "if" => parse_if_command_standalone(tokens, start, None, 0),
        "while" | "until" => parse_loop_command(tokens, start, None, 0),
        "case" => parse_case_command(tokens, start, None, 0),
        "select" => parse_select_command(tokens, start, None, 0),
        "coproc" => parse_coproc_command(tokens, start),
        "[[" => parse_conditional_command(tokens, start, None, 0),
        _ => None,
    }
}

fn is_brace_group_token(token: &Token) -> bool {
    token.kind == TokenKind::BraceExpand
        || (token.kind == TokenKind::Keyword
            && token.value.starts_with('{')
            && token.value.ends_with('}')
            && token.value.len() > 1)
}

fn parse_inline_brace_body(token: &Token) -> Vec<CommandNode> {
    let inner = token
        .value
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    // rubash#1544: folded-body re-lex under the fold pass's posix
    // mode, locked (see Token::posix_gate).
    let body_tokens = crate::lexer::tokenize_locked_body(inner, token.posix_gate);
    parse(&body_tokens).commands
}

fn parse_split_brace_body(tokens: &[Token], start: usize) -> Option<(Vec<CommandNode>, usize)> {
    if !is_keyword(tokens, start, "{") {
        return None;
    }

    let i = matching_brace_group_end(tokens, start)?;

    Some((parse(&tokens[start + 1..i]).commands, i))
}
