//! `taiji completion <shell>` — Shell 补全脚本生成

use crate::AppContext;
use anyhow::Result;

pub(crate) fn run(_ctx: AppContext, shell: String) -> Result<()> {
    match shell.to_lowercase().as_str() {
        "bash" => {
            println!("# taiji bash completion");
            println!("# 将以下内容添加到 ~/.bashrc 或运行: taiji completion bash > /etc/bash_completion.d/taiji");
            println!("complete -C taiji taiji");
        }
        "zsh" => {
            println!("# taiji zsh completion");
            println!("# 将以下内容添加到 ~/.zshrc:");
            println!("autoload -U +X bashcompinit && bashcompinit");
            println!("complete -C taiji taiji");
        }
        "fish" => {
            println!("# taiji fish completion");
            println!("# 将以下内容添加到 ~/.config/fish/completions/taiji.fish:");
            println!("complete -c taiji -f");
        }
        "powershell" => {
            println!("# taiji PowerShell completion");
            println!("# 将以下内容添加到你的 PowerShell profile:");
            println!("Register-ArgumentCompleter -Native -CommandName taiji -ScriptBlock {{ param($wordToComplete, $commandAst, $cursorPosition) }}");
        }
        _ => {
            eprintln!("不支持的 shell 类型: {}（支持: bash/zsh/fish/powershell）", shell);
        }
    }
    Ok(())
}
