use crate::error::{Error, Result};

pub fn run(shell: &str) -> Result<()> {
    match shell {
        "bash" | "zsh" | "sh" => {
            println!(
                r#"# nixy shell configuration
export PATH="$HOME/.local/state/nixy/env/bin:$PATH""#
            );
        }
        "fish" => {
            println!(
                r#"# nixy shell configuration
set -gx PATH $HOME/.local/state/nixy/env/bin $PATH"#
            );
        }
        "" => {
            return Err(Error::Usage(
                r#"Usage: nixy config <shell>
Supported shells: bash, zsh, fish

Add to your shell config:
  bash/zsh: eval "$(nixy config zsh)"
  fish:     nixy config fish | source"#
                    .to_string(),
            ));
        }
        _ => {
            return Err(Error::UnknownShell(shell.to_string()));
        }
    }

    Ok(())
}
