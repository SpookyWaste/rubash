//! Issue rubash#1657 (investigation pin): tilde expansion for a
//! system user that exists in the password database must resolve to
//! the database's home directory, and a name with no database entry
//! must stay unexpanded, verbatim.
//!
//! GNU subst.c tilde expansion (tilde.c) glues the getpwnam pw_dir
//! verbatim; rubash's owner is src/expand/tilde/tilde.rs
//! `passwd_home_for_user` (getpwnam_r on unix). The Termux report
//! behind this issue — a known-but-unlisted system user expanding to
//! `/` — has no in-tree root cause: no hardcoded system-name table
//! exists in src/, and on a glibc host rubash and GNU agree exactly,
//! because both read the same passwd database. The divergence enters
//! through the platform libc's synthetic passwd entries (bionic
//! returns pw_dir="/" for its synthetic users), which getpwnam_r
//! cannot distinguish from genuine entries — any code-level "fix"
//! would diverge from GNU's algorithm. This test pins the in-repo
//! invariant the implementation DOES own: faithful getpwnam lookup,
//! verified against the host's own /etc/passwd.
//!
//! Expectations verified against GNU bash 5.3.20 (self-compiled
//! oracle) on this host.

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

fn passwd_home(user: &str) -> Option<String> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        let mut fields = line.split(':');
        if fields.next() == Some(user) {
            // passwd(5) layout: name:passwd:uid:gid:gecos:home:shell —
            // home is the sixth field. (An earlier version of this
            // helper skipped one field too few and returned the gecos
            // field — "root" for root — as the expected home.)
            let _passwd = fields.next();
            let _uid = fields.next();
            let _gid = fields.next();
            let _gecos = fields.next();
            return fields.next().map(str::to_owned);
        }
    }
    None
}

#[test]
fn issue1657_tilde_unknown_user_stays_verbatim() {
    let (stdout, _, _) = rubash("echo ~nosuchuser1657xyz; echo ~nosuchuser1657xyz/sub");
    assert_eq!(stdout, "~nosuchuser1657xyz\n~nosuchuser1657xyz/sub\n");
}

#[test]
fn issue1657_tilde_known_users_match_passwd_database() {
    // root plus the first listed system accounts: whatever the host
    // database says the home is, the expansion must glue it verbatim.
    let passwd = std::fs::read_to_string("/etc/passwd").expect("host /etc/passwd");
    let mut checked = 0;
    for line in passwd.lines().take(4) {
        let Some(user) = line.split(':').next() else {
            continue;
        };
        let Some(home) = passwd_home(user).filter(|home| !home.is_empty()) else {
            continue;
        };
        let (stdout, _, _) = rubash(&format!("echo ~{user}; echo ~{user}/sub"));
        assert_eq!(stdout, format!("{home}\n{home}/sub\n"), "user {user}");
        checked += 1;
    }
    assert!(checked >= 2, "expected at least root + one system user");
}
