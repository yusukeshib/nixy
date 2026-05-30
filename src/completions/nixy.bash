# nixy bash completion
# Dynamic completion candidates are produced by the hidden `nixy completions`
# helper, so installed package names and profiles always stay in sync.

_nixy() {
    local cur prev cmd
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local subcommands="install add uninstall remove list ls search update sync config profile upgrade file"

    if [[ $COMP_CWORD -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "$subcommands" -- "$cur") )
        return
    fi

    cmd="${COMP_WORDS[1]}"
    case "$cmd" in
        uninstall|remove|file)
            COMPREPLY=( $(compgen -W "$(command nixy completions installed 2>/dev/null)" -- "$cur") )
            ;;
        update)
            COMPREPLY=( $(compgen -W "--all $(command nixy completions installed 2>/dev/null)" -- "$cur") )
            ;;
        install|add)
            if [[ "$cur" == -* || "$prev" == "-p" || "$prev" == "--platform" ]]; then
                COMPREPLY=( $(compgen -W "-p --platform darwin macos linux x86_64-darwin aarch64-darwin x86_64-linux aarch64-linux" -- "$cur") )
            fi
            ;;
        config)
            COMPREPLY=( $(compgen -W "zsh bash fish" -- "$cur") )
            ;;
        profile)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=( $(compgen -W "-c -d" -- "$cur") )
            else
                COMPREPLY=( $(compgen -W "$(command nixy completions profiles 2>/dev/null)" -- "$cur") )
            fi
            ;;
        upgrade)
            COMPREPLY=( $(compgen -W "-f --force" -- "$cur") )
            ;;
    esac
}

complete -F _nixy nixy
