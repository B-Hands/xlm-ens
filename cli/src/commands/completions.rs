use clap::{CommandFactory, Subcommand};
use clap_complete::{Shell, generate, generate_to};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Subcommand)]
pub enum CompletionCommand {
    /// Generate a bash completion script to stdout.
    Bash,
    /// Generate a zsh completion script to stdout.
    Zsh,
    /// Generate a fish completion script to stdout.
    Fish,
    /// Install completions for the detected shell into the standard user directory.
    Install,
}

pub fn run_completion_command<C: CommandFactory>(
    command: CompletionCommand,
    bin_name: &str,
) -> anyhow::Result<()> {
    match command {
        CompletionCommand::Bash => generate_completion_script::<C>(Shell::Bash, bin_name),
        CompletionCommand::Zsh => generate_completion_script::<C>(Shell::Zsh, bin_name),
        CompletionCommand::Fish => generate_completion_script::<C>(Shell::Fish, bin_name),
        CompletionCommand::Install => install_completion::<C>(bin_name),
    }
}

fn generate_completion_script<C: CommandFactory>(
    shell: Shell,
    bin_name: &str,
) -> anyhow::Result<()> {
    let mut cmd = C::command();
    generate(shell, &mut cmd, bin_name, &mut io::stdout());
    Ok(())
}

fn install_completion<C: CommandFactory>(bin_name: &str) -> anyhow::Result<()> {
    let shell = detect_shell().ok_or_else(|| {
        anyhow::anyhow!(
            "could not detect your shell; run `xlm-ns completions bash|zsh|fish` instead"
        )
    })?;
    let install_dir = completion_install_dir(shell)
        .ok_or_else(|| anyhow::anyhow!("could not determine your home directory"))?;

    fs::create_dir_all(&install_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create completion directory {}: {err}",
            install_dir.display()
        )
    })?;

    let mut cmd = C::command();
    let installed = generate_to(shell, &mut cmd, bin_name, &install_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to install completion script into {}: {err}",
            install_dir.display()
        )
    })?;

    println!(
        "Installed {} completion script to {}",
        shell_name(shell),
        installed.display()
    );
    Ok(())
}

fn detect_shell() -> Option<Shell> {
    let shell = env::var_os("SHELL")
        .or_else(|| env::var_os("ComSpec"))
        .and_then(|value| value.into_string().ok())?;
    shell_from_path(&shell)
}

fn shell_from_path(value: &str) -> Option<Shell> {
    let name = Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())?
        .to_ascii_lowercase();

    match name.trim_end_matches(".exe") {
        "bash" => Some(Shell::Bash),
        "zsh" => Some(Shell::Zsh),
        "fish" => Some(Shell::Fish),
        _ => None,
    }
}

fn completion_install_dir(shell: Shell) -> Option<PathBuf> {
    home_dir().map(|home| completion_install_dir_from_home(&home, shell))
}

fn completion_install_dir_from_home(home: &Path, shell: Shell) -> PathBuf {
    match shell {
        Shell::Bash => home
            .join(".local")
            .join("share")
            .join("bash-completion")
            .join("completions"),
        Shell::Zsh => home
            .join(".local")
            .join("share")
            .join("zsh")
            .join("site-functions"),
        Shell::Fish => home.join(".config").join("fish").join("completions"),
        _ => unreachable!("only bash, zsh, and fish are supported"),
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn shell_name(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => "bash",
        Shell::Zsh => "zsh",
        Shell::Fish => "fish",
        _ => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_shells_from_paths() {
        assert_eq!(shell_from_path("/bin/bash"), Some(Shell::Bash));
        assert_eq!(shell_from_path("/usr/bin/zsh"), Some(Shell::Zsh));
        assert_eq!(shell_from_path("/usr/bin/fish"), Some(Shell::Fish));
        assert_eq!(shell_from_path("/usr/bin/sh"), None);
    }

    #[test]
    fn builds_shell_specific_install_dirs() {
        let home = Path::new("/home/alice");
        assert_eq!(
            completion_install_dir_from_home(home, Shell::Bash),
            PathBuf::from("/home/alice/.local/share/bash-completion/completions")
        );
        assert_eq!(
            completion_install_dir_from_home(home, Shell::Zsh),
            PathBuf::from("/home/alice/.local/share/zsh/site-functions")
        );
        assert_eq!(
            completion_install_dir_from_home(home, Shell::Fish),
            PathBuf::from("/home/alice/.config/fish/completions")
        );
    }
}
