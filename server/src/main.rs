//! incipit: ticket server (MCP), permission guard (hook) and owner CLI in one binary.

mod admin;
mod config;
mod decisions;
mod guard;
mod mcp;
mod owner;
mod spec;
mod store;
mod tools;
mod util;

use std::process::exit;

const USAGE: &str = "incipit <command>

  mcp                      run the ticket server (MCP over stdio)
  guard                    PreToolUse hook: reads the hook event on stdin
  phase [show]             show the setup phases
  phase done|reset <name>  mark a setup phase (scaffold, grill, toolchain, team, activate)
  activate | deactivate    make the lead the main session agent, or undo it
  status                   tickets, proposals, open questions, and the steps that are yours
  ticket list [status] | show <id>
  ticket withdraw <id> <reason>        take a ticket out of the queue
  ticket role <id> implementer|tester  set who builds an open ticket
  ticket verify            check all closed tickets against their receipts
  spec list | show <id>    spec change proposals filed by the architect
  spec apply <id>          apply a proposal to SPEC.md
  spec reject <id> [reason]
  decisions [open|all]     the decision log kept by the lead
  role add security|researcher         switch an optional role on
  upgrade                  bring a project scaffolded by an older version up to date
  version
";

const GUARD_HELP: &str = "incipit guard

  PreToolUse hook for Claude Code. Takes no arguments. Reads the hook event
  (JSON) on stdin and prints a deny decision if the call breaks the role
  rules in .incipit/config.json; prints nothing if the call is allowed.

  Try it:  echo '{\"tool_name\":\"Edit\",\"agent_type\":\"implementer\",\"tool_input\":{\"file_path\":\"tests/x\"}}' | incipit guard
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = util::project_root();
    let outcome = match args.first().map(String::as_str) {
        Some("mcp") => {
            mcp::serve(&root);
            return;
        }
        Some("guard") => {
            // Any argument means a person is asking; the hook passes none.
            if args.len() > 1 {
                print!("{GUARD_HELP}");
                return;
            }
            guard::run(&root);
            return;
        }
        Some("phase") => admin::phase(&root, &args[1..]),
        Some("ticket") => owner::ticket(&root, &args[1..]),
        Some("spec") => spec::cli(&root, &args[1..]),
        Some("upgrade") => owner::upgrade(&root),
        Some("status") => owner::status(&root),
        Some("decisions") => decisions::cli(&root, &args[1..]),
        Some("role") => owner::role(&root, &args[1..]),
        Some("activate") => admin::set_lead(&root, true),
        Some("deactivate") => admin::set_lead(&root, false),
        Some("version") => Ok(format!("incipit {}", env!("CARGO_PKG_VERSION"))),
        _ => {
            eprint!("{USAGE}");
            exit(64);
        }
    };
    match outcome {
        Ok(text) => print!("{}{}", text, if text.ends_with('\n') { "" } else { "\n" }),
        Err(text) => {
            eprintln!("incipit: {}", text.trim_end());
            exit(1);
        }
    }
}
