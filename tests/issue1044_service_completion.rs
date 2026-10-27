//! Issue rubash#1044 regression: `compgen -A service` was always empty
//! on hosts without a readable services file.
//!
//! GNU bashline.c:2667 bash_servicename_completion_function walks the
//! libc getservent() database in resolver order — which includes
//! resolver-local fallbacks (e.g. bionic's built-in table) that no
//! on-disk file parse can see. An entry contributes its primary name
//! when it matches the word prefix, otherwise the first matching alias
//! — never both — and tcp/udp duplicates are kept (pcomplete.c:820
//! GEN_XCOMPS appends without dedup). GNU returns NULL unconditionally
//! on Windows, so the candidate list is empty there.

use std::process::Command;

fn rubash(script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

/// Re-derive the expected candidates from the live libc getservent()
/// database under the bash_servicename_completion_function rule, so the
/// assertion survives any resolver contents (on-disk file, nsswitch
/// sources, or a libc built-in fallback table).
#[cfg(unix)]
fn expected_candidates(word: &str) -> Vec<String> {
    use std::ffi::CStr;
    let mut out = Vec::new();
    // SAFETY: single-threaded test; every pointer is copied before the
    // next getservent call, mirroring the builtin under test.
    unsafe {
        libc::setservent(0);
        loop {
            let entry = libc::getservent();
            if entry.is_null() {
                break;
            }
            let name = CStr::from_ptr((*entry).s_name);
            let name = String::from_utf8_lossy(name.to_bytes()).into_owned();
            if name.starts_with(word) {
                out.push(name);
                continue;
            }
            let mut aliases = (*entry).s_aliases;
            while !aliases.is_null() && !(*aliases).is_null() {
                let alias = CStr::from_ptr(*aliases);
                let alias = String::from_utf8_lossy(alias.to_bytes()).into_owned();
                if alias.starts_with(word) {
                    out.push(alias);
                    break;
                }
                aliases = aliases.add(1);
            }
        }
        libc::endservent();
    }
    out
}

#[test]
fn service_candidates_come_from_the_system_database() {
    let (stdout, _, rc) = rubash("compgen -A service");
    let got: Vec<String> = stdout.lines().map(str::to_string).collect();
    #[cfg(unix)]
    {
        let expected = expected_candidates("");
        if expected.is_empty() {
            // Resolver genuinely empty: compgen_builtin reports failure
            // (complete.def:762-777) on an empty match list.
            assert!(got.is_empty());
            assert_eq!(rc, Some(1));
        } else {
            assert_eq!(rc, Some(0));
            assert_eq!(got, expected, "candidate list must follow database order");
        }
    }
    #[cfg(not(unix))]
    {
        // GNU bashline.c returns NULL on Windows: no candidates, failure.
        assert!(got.is_empty());
        assert_eq!(rc, Some(1));
    }
}

#[test]
fn service_prefix_and_alias_matching() {
    let (stdout, _, _) = rubash("compgen -A service ftp");
    let got: Vec<String> = stdout.lines().map(str::to_string).collect();
    #[cfg(unix)]
    {
        let expected = expected_candidates("ftp");
        assert_eq!(
            got, expected,
            "prefix/alias matches must follow getservent rules"
        );
        // A non-empty resolver lists strictly more unfiltered candidates
        // than `ftp`-filtered ones.
        if !expected.is_empty() {
            let (all, _, _) = rubash("compgen -A service");
            assert!(all.lines().count() > got.len());
        }
    }
    #[cfg(not(unix))]
    {
        assert!(got.is_empty());
    }
}
