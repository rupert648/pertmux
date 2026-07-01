use crate::daemon;
use crate::protocol::{ClientMsg, CodexHookEvent};
use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

const HOOK_EVENTS: [&str; 3] = ["SessionStart", "UserPromptSubmit", "Stop"];

pub async fn send_from_stdin(verbose: bool) -> Result<()> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("failed to read Codex hook payload from stdin")?;

    if input.trim().is_empty() {
        if verbose {
            eprintln!("pertmux codex-hook: empty stdin");
        }
        return Ok(());
    }

    let event: CodexHookEvent =
        serde_json::from_str(&input).context("failed to parse Codex hook payload")?;

    let sock_path = daemon::socket_path();
    let Ok(stream) = UnixStream::connect(&sock_path).await else {
        if verbose {
            eprintln!(
                "pertmux codex-hook: daemon not running at {}",
                sock_path.display()
            );
        }
        return Ok(());
    };

    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    // The daemon sends the latest snapshot immediately on connect. Drain it so
    // the command channel behaves like the existing `pertmux stop` helper.
    let _ = framed.next().await;

    let msg = ClientMsg::CodexHook(Box::new(event));
    framed
        .send(Bytes::from(serde_json::to_vec(&msg)?))
        .await
        .context("failed to send Codex hook event to pertmux daemon")?;

    Ok(())
}

pub fn install(local: bool, repo: Option<PathBuf>, force: bool) -> Result<()> {
    let (codex_dir, scope) = if local {
        let repo_root = match repo {
            Some(path) => path,
            None => std::env::current_dir().context("failed to resolve current directory")?,
        };
        let repo_root = std::fs::canonicalize(&repo_root).unwrap_or(repo_root);
        (repo_root.join(".codex"), "local")
    } else {
        if repo.is_some() {
            anyhow::bail!("--repo can only be used with --local");
        }
        let home = dirs::home_dir().context("failed to resolve home directory")?;
        (home.join(".codex"), "global")
    };

    std::fs::create_dir_all(&codex_dir)
        .with_context(|| format!("failed to create {}", codex_dir.display()))?;

    let hooks_path = codex_dir.join("hooks.json");
    let mut root = if hooks_path.exists() {
        let raw = std::fs::read_to_string(&hooks_path)
            .with_context(|| format!("failed to read {}", hooks_path.display()))?;
        match serde_json::from_str::<Value>(&raw) {
            Ok(value) => value,
            Err(err) if force => {
                eprintln!(
                    "warning: replacing invalid {} ({})",
                    hooks_path.display(),
                    err
                );
                json!({})
            }
            Err(err) => {
                anyhow::bail!(
                    "failed to parse {}; rerun with --force to replace it: {}",
                    hooks_path.display(),
                    err
                );
            }
        }
    } else {
        json!({})
    };

    merge_pertmux_hooks(&mut root)?;

    let rendered = serde_json::to_string_pretty(&root)?;
    std::fs::write(&hooks_path, format!("{rendered}\n"))
        .with_context(|| format!("failed to write {}", hooks_path.display()))?;

    crate::banner::print();
    println!("  installed Codex hooks");
    println!();
    println!("  scope   {scope}");
    println!("  hooks   {}", hooks_path.display());
    println!("  command {}", hook_command()?);
    println!();
    if local {
        println!("  Start Codex in this repo and run /hooks once to review and trust them.");
    } else {
        println!("  Start Codex and run /hooks once to review and trust them.");
    }
    println!("  For one-off testing, use: codex --dangerously-bypass-hook-trust");
    println!();

    Ok(())
}

fn merge_pertmux_hooks(root: &mut Value) -> Result<()> {
    if !root.is_object() {
        *root = json!({});
    }

    let root_obj = root.as_object_mut().expect("root object ensured");
    let hooks_value = root_obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks_value.is_object() {
        anyhow::bail!("hooks.json has a non-object `hooks` field");
    }

    let hooks_obj = hooks_value.as_object_mut().expect("hooks object ensured");
    for event in HOOK_EVENTS {
        let entry = hooks_obj.entry(event).or_insert_with(|| json!([]));
        if !entry.is_array() {
            anyhow::bail!("hooks.json field `hooks.{event}` must be an array");
        }

        let groups = entry.as_array_mut().expect("event array ensured");
        groups.retain(|group| !is_pertmux_group(group));
        groups.push(pertmux_group(event)?);
    }

    Ok(())
}

fn pertmux_group(event: &str) -> Result<Value> {
    let mut group = json!({
        "hooks": [
            {
                "type": "command",
                "command": hook_command()?,
                "timeout": 5,
                "statusMessage": "Notifying pertmux"
            }
        ]
    });

    if event == "SessionStart" {
        group["matcher"] = json!("startup|resume");
    }

    Ok(group)
}

fn is_pertmux_group(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|cmd| cmd.contains("pertmux") && cmd.contains("codex-hook"))
            })
        })
}

fn hook_command() -> Result<String> {
    let exe = std::env::current_exe().context("failed to resolve pertmux executable")?;
    Ok(format!("{} codex-hook", shell_quote(&exe)))
}

fn shell_quote(path: &Path) -> String {
    let s = path.to_string_lossy();
    format!("'{}'", s.replace('\'', "'\\''"))
}
