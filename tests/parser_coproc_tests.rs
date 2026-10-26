use rubash::lexer::tokenize;
use rubash::parser::{parse, CoprocBodyKind, LoopKind, QuoteKind};

#[test]
fn test_coproc_records_script_line() {
    let ast = parse(&tokenize("#!/usr/bin/env bash\n\ncoproc worker true\n"));
    assert_eq!(ast.commands[0].line, Some(3));
}

#[test]
fn test_named_coproc_parses_split_brace_group_body() {
    let input = "coproc MYC { echo hi; }";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.keyword, "coproc");
    assert_eq!(coproc.keyword_metadata.value, "coproc");
    assert_eq!(coproc.keyword_metadata.raw, "coproc");
    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    let name_metadata = coproc.name_metadata.as_ref().unwrap();
    assert_eq!(name_metadata.word_index, 0);
    assert_eq!(name_metadata.value, "MYC");
    assert_eq!(name_metadata.raw, "MYC");
    assert!(name_metadata.word_quotes.is_empty());
    assert!(coproc.words.is_empty());
    assert_eq!(coproc.body_kind, CoprocBodyKind::BraceGroup);
    assert_eq!(coproc.body_open_delimiter.as_deref(), Some("{"));
    assert_eq!(
        coproc.body_open_delimiter_metadata.as_ref().unwrap().value,
        "{"
    );
    assert_eq!(coproc.body_close_delimiter.as_deref(), Some("}"));
    assert_eq!(
        coproc.body_close_delimiter_metadata.as_ref().unwrap().raw,
        "}"
    );
    let body = coproc.body.as_ref().unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0].words, ["echo", "hi"]);
}

#[test]
fn test_unnamed_coproc_parses_split_brace_group_body() {
    let input = "coproc { echo hi; }";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.keyword, "coproc");
    assert_eq!(coproc.name, None);
    assert!(coproc.name_metadata.is_none());
    assert!(coproc.words.is_empty());
    assert_eq!(coproc.body_kind, CoprocBodyKind::BraceGroup);
    assert_eq!(coproc.body_open_delimiter.as_deref(), Some("{"));
    assert_eq!(coproc.body_close_delimiter.as_deref(), Some("}"));
    assert_eq!(coproc.body.as_ref().unwrap()[0].words, ["echo", "hi"]);
}

#[test]
fn test_coproc_brace_body_keeps_case_pattern_named_like_close_brace() {
    let input = "coproc { case brace in }) echo close ;; esac; echo after; }";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let body = coproc.body.as_ref().unwrap();

    assert_eq!(
        body[0].case_command.as_ref().unwrap().clauses[0].patterns,
        ["}"]
    );
    assert_eq!(body[1].words, ["echo", "after"]);
}

#[test]
fn test_coproc_simple_command_does_not_treat_first_word_as_name() {
    let input = "coproc MYC cat";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.keyword, "coproc");
    assert_eq!(coproc.name, None);
    assert!(coproc.name_metadata.is_none());
    assert_eq!(coproc.words, ["MYC", "cat"]);
    assert_eq!(coproc.body_kind, CoprocBodyKind::SimpleCommand);
    assert_eq!(coproc.body_open_delimiter, None);
    assert!(coproc.body_open_delimiter_metadata.is_none());
    assert_eq!(coproc.body_close_delimiter, None);
    assert!(coproc.body_close_delimiter_metadata.is_none());
    assert!(coproc.body.is_none());
}

#[test]
fn test_coproc_simple_command_keeps_process_substitution_words() {
    let input = "coproc cat <(:) >(:)";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name, None);
    assert_eq!(coproc.words, ["cat", "<(:)", ">(:)"]);
    assert_eq!(coproc.body_kind, CoprocBodyKind::SimpleCommand);
}

#[test]
fn test_coproc_simple_command_records_word_metadata() {
    let input = "coproc echo $value \"*.rs\" pre{a,b}";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    // wt33 (#373): the quoted `*.rs` carries the \x11 CTLESC carrier in
    // token.value (execution strips it — probe wt33-373/run/A28_coproc_words
    // family; the body-output blank-line leak is tracked by rubash#387).
    assert_eq!(coproc.words, ["echo", "$value", "\u{11}*.rs", "pre{a,b}"]);
    assert_eq!(coproc.word_metadata.len(), 4);
    assert_eq!(coproc.word_metadata[1].value, "$value");
    // wt44/parse4: parameter / brace records are no longer populated at
    // parse time (zero readers; the executor re-derives from word text).

    assert_eq!(coproc.word_metadata[2].raw, "\"*.rs\"");
    assert!(coproc.word_metadata[2].pathname_patterns.is_empty());
    // wt44/parse4: word-quote records re-derive at the consumer.
    assert_eq!(
        rubash::parser::word_quotes_in_raw_public(&coproc.word_metadata[2].raw)[0].kind,
        QuoteKind::Double
    );
}

#[test]
fn test_coproc_simple_command_keeps_raw_quoted_arguments() {
    let input = "coproc printf '<%s>\\n' 'a b'";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();

    assert_eq!(coproc.words, ["printf", "<%s>\\n", "a b"]);
    assert_eq!(coproc.word_metadata[1].raw, "'<%s>\\n'");
    assert_eq!(coproc.word_metadata[2].raw, "'a b'");
}

#[test]
fn test_coproc_simple_command_keeps_reserved_word_arguments() {
    let input = "coproc echo alpha if done esac";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name, None);
    assert_eq!(coproc.words, ["echo", "alpha", "if", "done", "esac"]);
    assert_eq!(coproc.body_kind, CoprocBodyKind::SimpleCommand);
}

#[test]
fn test_named_coproc_parses_subshell_body() {
    let input = "coproc MYC ( echo hi )";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::Subshell);
    assert_eq!(coproc.body_open_delimiter.as_deref(), Some("("));
    assert_eq!(
        coproc.body_open_delimiter_metadata.as_ref().unwrap().value,
        "("
    );
    assert_eq!(coproc.body_close_delimiter.as_deref(), Some(")"));
    assert_eq!(
        coproc.body_close_delimiter_metadata.as_ref().unwrap().value,
        ")"
    );
    assert_eq!(coproc.body.as_ref().unwrap()[0].words, ["echo", "hi"]);
}

#[test]
fn test_named_coproc_subshell_keeps_case_pattern_parentheses() {
    let input = "coproc MYC ( case beta in alpha) printf alpha ;; beta) printf beta ;; esac )";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::Subshell);
    assert_eq!(coproc.body_open_delimiter.as_deref(), Some("("));
    assert_eq!(coproc.body_close_delimiter.as_deref(), Some(")"));
    assert!(coproc.body.as_ref().unwrap()[0].case_command.is_some());
}

#[test]
fn test_named_coproc_subshell_keeps_case_argument() {
    let input = "coproc MYC ( echo case; echo ok )";
    let tokens = tokenize(input);
    let ast = parse(&tokens);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let body = coproc.body.as_ref().unwrap();

    assert_eq!(coproc.body_kind, CoprocBodyKind::Subshell);
    assert_eq!(body[0].words, ["echo", "case"]);
    assert_eq!(body[1].words, ["echo", "ok"]);
}

#[test]
fn test_named_coproc_parses_for_body() {
    let input = "coproc MYC for x in a b; do echo $x; done";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert!(coproc.words.is_empty());
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    let for_command = coproc.body.as_ref().unwrap()[0]
        .for_command
        .as_ref()
        .unwrap();
    assert_eq!(for_command.variable, "x");
    assert_eq!(for_command.words, ["a", "b"]);
    assert_eq!(for_command.body[0].words, ["echo", "$x"]);
}

#[test]
fn test_coproc_time_word_starts_the_simple_command() {
    // GNU parse.y:1125-1173: the coproc productions are only
    // `COPROC [WORD] shell_command` and `COPROC simple_command` —
    // a WORD is a name only in front of a shell_command, so here
    // `MYC` is the first word of the unnamed coproc's simple
    // command and `time` the second (oracle: `coproc MYC time
    // echo hi` runs a command named MYC and reports it not found).
    // The coproc is stage 0 of the pipeline (rubash#1653).
    let input = "coproc MYC time echo hi | wc -c";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let pipeline = ast.commands[0].pipeline_command.as_ref().unwrap();
    assert_eq!(pipeline.operators, ["|"]);
    let coproc = pipeline.stages[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name, None);
    assert_eq!(coproc.body_kind, CoprocBodyKind::SimpleCommand);
    assert!(coproc.body.is_none());
    assert_eq!(coproc.words, ["MYC", "time", "echo", "hi"]);
    assert_eq!(pipeline.stages[1].words, ["wc", "-c"]);
}

#[test]
fn test_coproc_bang_after_one_word_is_a_parse_error() {
    // GNU's bang rule for a coproc operand (parse.y: BANG belongs
    // to pipeline grammar): with exactly one word collected —
    // here `time` — the `!` sits where the command name would
    // continue and the line is a syntax error (rubash#1652). With
    // two words in front of it (`coproc MYC time ! ...`) the `!`
    // is an ordinary argument word and the line parses.
    let ast = parse(&tokenize("coproc time ! echo hi"));
    assert!(ast.commands[0].coproc_command.is_none());
    assert_eq!(
        ast.commands[0]
            .get_assignment("__RUBASH_PARSE_ERROR__")
            .map(String::as_str),
        Some("unexpected token `!'")
    );

    let ast = parse(&tokenize("coproc MYC time ! echo hi"));
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name, None);
    assert_eq!(coproc.words, ["MYC", "time", "!", "echo", "hi"]);
}

#[test]
fn test_coproc_time_simple_command_ends_at_pipeline_bar() {
    // Whatever compound stage follows the `|`, the coproc itself
    // is the simple command `MYC time echo hi` in stage 0: GNU has
    // no timed coproc for a later stage to attach to
    // (rubash#1653).
    for input in [
        "coproc MYC time echo hi | if read value; then echo $value; fi",
        "coproc MYC time echo hi | ( read value; echo $value )",
        "coproc MYC time echo hi | { read value; echo $value; }",
    ] {
        let tokens = tokenize(input);
        let ast = parse(&tokens);
        let pipeline = ast.commands[0].pipeline_command.as_ref().expect(input);
        let coproc = pipeline.stages[0].coproc_command.as_ref().expect(input);
        assert_eq!(coproc.name, None, "{input}");
        assert_eq!(coproc.body_kind, CoprocBodyKind::SimpleCommand, "{input}");
        assert_eq!(coproc.words, ["MYC", "time", "echo", "hi"], "{input}");
    }
}

#[test]
fn test_coproc_time_words_collect_like_a_simple_command() {
    // `coproc MYC time -p echo hi` is one simple command whose
    // words collect verbatim (flags and mid-command words alike);
    // in the `for` line the collected words end at the `;` and the
    // stray `do` then fails the whole line — the exact diagnostic
    // GNU gives for it (rubash#1653).
    let ast = parse(&tokenize("coproc MYC time -p echo hi"));
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name, None);
    assert_eq!(coproc.body_kind, CoprocBodyKind::SimpleCommand);
    assert_eq!(coproc.words, ["MYC", "time", "-p", "echo", "hi"]);

    let ast = parse(&tokenize("coproc MYC time -p for x in a; do echo $x; done"));
    assert_eq!(
        ast.commands[0]
            .get_assignment("__RUBASH_PARSE_ERROR__")
            .map(String::as_str),
        Some("unexpected token `do'")
    );
}

#[test]
fn test_named_coproc_parses_function_body() {
    let input = "coproc MYC foo() { echo hi; }";
    let tokens = tokenize(input);
    let ast = parse(&tokens);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let function = coproc.body.as_ref().unwrap()[0]
        .function_command
        .as_ref()
        .unwrap();

    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(function.name, "foo");
    assert!(function.has_parentheses);
    assert_eq!(function.body[0].words, ["echo", "hi"]);
}

#[test]
fn test_coproc_time_word_before_function_definition() {
    // There is no timed function-body coproc in GNU's grammar
    // either: `MYC` and `time` are the first two words of the
    // unnamed coproc's simple command (GNU then rejects the line
    // at the `(`; rubash's verbatim word collection carries the
    // remaining text as words — the trailing-token divergence
    // family). What this pins is that no time_command body and no
    // name binding appear (rubash#1653).
    let input = "coproc MYC time foo() { echo hi; }";
    let tokens = tokenize(input);
    let ast = parse(&tokens);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();

    assert_eq!(coproc.name, None);
    assert_eq!(coproc.body_kind, CoprocBodyKind::SimpleCommand);
    assert!(coproc.body.is_none());
    assert_eq!(coproc.words[..2], ["MYC", "time"]);
}

#[test]
fn test_named_coproc_parses_arithmetic_body() {
    let input = "coproc MYC (( 1 + 1 ))";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert!(coproc.words.is_empty());
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    let arithmetic = coproc.body.as_ref().unwrap()[0]
        .arithmetic_command
        .as_ref()
        .unwrap();
    assert_eq!(arithmetic.expression, "1 + 1");
}

#[test]
fn test_named_coproc_parses_conditional_body() {
    let input = "coproc MYC [[ $value == ok ]]";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert!(coproc.words.is_empty());
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    let conditional = coproc.body.as_ref().unwrap()[0]
        .conditional_command
        .as_ref()
        .unwrap();
    assert_eq!(conditional.args, ["$value", "==", "ok", "]]"]);
}

#[test]
fn test_named_coproc_parses_case_body() {
    let input = "coproc MYC case beta in alpha) echo alpha ;; beta) echo beta ;; esac";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let case_command = coproc.body.as_ref().unwrap()[0]
        .case_command
        .as_ref()
        .unwrap();

    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(case_command.word, "beta");
    assert_eq!(case_command.clauses.len(), 2);
    assert_eq!(case_command.clauses[1].patterns, ["beta"]);
    assert_eq!(case_command.clauses[1].body[0].words, ["echo", "beta"]);
}

#[test]
fn test_named_coproc_parses_if_body() {
    let input = "coproc MYC if true; then echo yes; else echo no; fi";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let if_command = coproc.body.as_ref().unwrap()[0]
        .if_command
        .as_ref()
        .unwrap();

    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(if_command.condition[0].words, ["true"]);
    assert_eq!(if_command.then_body[0].words, ["echo", "yes"]);
    assert_eq!(
        if_command.else_body.as_ref().unwrap()[0].words,
        ["echo", "no"]
    );
}

#[test]
fn test_named_coproc_parses_while_body() {
    let input = "coproc MYC while true; do echo loop; break; done";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let loop_command = coproc.body.as_ref().unwrap()[0]
        .loop_command
        .as_ref()
        .unwrap();

    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(loop_command.kind, LoopKind::While);
    assert_eq!(loop_command.condition[0].words, ["true"]);
    assert_eq!(loop_command.body[0].words, ["echo", "loop"]);
    assert_eq!(loop_command.body[1].words, ["break"]);
}

#[test]
fn test_named_coproc_parses_until_body() {
    let input = "coproc MYC until false; do echo loop; break; done";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let loop_command = coproc.body.as_ref().unwrap()[0]
        .loop_command
        .as_ref()
        .unwrap();

    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(loop_command.kind, LoopKind::Until);
    assert_eq!(loop_command.condition[0].words, ["false"]);
    assert_eq!(loop_command.body[0].words, ["echo", "loop"]);
    assert_eq!(loop_command.body[1].words, ["break"]);
}

#[test]
fn test_named_coproc_parses_select_body() {
    let input = "coproc MYC select choice in alpha beta; do echo $choice; break; done";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let select = coproc.body.as_ref().unwrap()[0]
        .select_command
        .as_ref()
        .unwrap();

    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(select.variable, "choice");
    assert_eq!(select.words, ["alpha", "beta"]);
    assert_eq!(select.body[0].words, ["echo", "$choice"]);
}

#[test]
fn test_coproc_conditional_body_keeps_quoted_closing_delimiter_word() {
    let input = "coproc MYC [[ value == \"]]\" ]]";
    let tokens = tokenize(input);
    let ast = parse(&tokens);
    let coproc = ast.commands[0].coproc_command.as_ref().unwrap();
    let conditional = coproc.body.as_ref().unwrap()[0]
        .conditional_command
        .as_ref()
        .unwrap();

    assert_eq!(coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(conditional.args, ["value", "==", "]]", "]]"]);
    assert_eq!(conditional.expression.operands, ["value", "]]"]);
}

#[test]
fn test_coproc_sequence_body_keeps_reserved_word_arguments() {
    let if_tokens = tokenize("coproc MYC if echo then; then echo fi; fi");
    let if_ast = parse(&if_tokens);
    let if_coproc = if_ast.commands[0].coproc_command.as_ref().unwrap();
    let if_command = if_coproc.body.as_ref().unwrap()[0]
        .if_command
        .as_ref()
        .unwrap();

    let loop_tokens = tokenize("coproc MYC while echo do; do echo done; break; done");
    let loop_ast = parse(&loop_tokens);
    let loop_coproc = loop_ast.commands[0].coproc_command.as_ref().unwrap();
    let loop_command = loop_coproc.body.as_ref().unwrap()[0]
        .loop_command
        .as_ref()
        .unwrap();

    let until_tokens = tokenize("coproc MYC until echo do; do echo done; break; done");
    let until_ast = parse(&until_tokens);
    let until_coproc = until_ast.commands[0].coproc_command.as_ref().unwrap();
    let until_command = until_coproc.body.as_ref().unwrap()[0]
        .loop_command
        .as_ref()
        .unwrap();

    assert_eq!(if_coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(if_command.condition[0].words, ["echo", "then"]);
    assert_eq!(if_command.then_body[0].words, ["echo", "fi"]);
    assert_eq!(loop_coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert_eq!(loop_command.condition[0].words, ["echo", "do"]);
    assert_eq!(loop_command.body[0].words, ["echo", "done"]);
    assert_eq!(until_coproc.body_kind, CoprocBodyKind::CompoundCommand);
    assert!(until_command.until);
    assert_eq!(until_command.condition[0].words, ["echo", "do"]);
    assert_eq!(until_command.body[0].words, ["echo", "done"]);
}

#[test]
fn test_coproc_command_consumes_pipe_stderr_operator() {
    let input = "coproc MYC { echo hi; } |& grep hi";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let pipeline = ast.commands[0].pipeline_command.as_ref().unwrap();
    assert_eq!(pipeline.operators, ["|&"]);
    assert_eq!(pipeline.stages[0].pipe, Some(2));
    assert!(pipeline.stages[0].coproc_command.is_some());
    assert_eq!(pipeline.stages[1].words, ["grep", "hi"]);
}

#[test]
fn test_coproc_command_consumes_and_or_connector() {
    let input = "coproc MYC { echo hi; } && echo done";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let list = ast.commands[0].and_or_list.as_ref().unwrap();
    assert_eq!(list.connectors, [true]);
    assert!(list.commands[0].coproc_command.is_some());
    assert_eq!(list.commands[1].words, ["echo", "done"]);
}

#[test]
fn test_coproc_command_consumes_trailing_stdout_redirect() {
    let input = "coproc MYC { echo hi; } > out.txt";
    let tokens = tokenize(input);
    let ast = parse(&tokens);

    assert_eq!(ast.commands.len(), 1);
    let command = &ast.commands[0];
    let coproc = command.coproc_command.as_ref().unwrap();
    assert_eq!(coproc.name.as_deref(), Some("MYC"));
    assert_eq!(coproc.body_kind, CoprocBodyKind::BraceGroup);
    assert_eq!(command.redirect_out.as_ref().unwrap().target, "out.txt");
}
